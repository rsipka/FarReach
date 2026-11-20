use super::vv::parse;
use super::*;

#[test]
fn proxmox_vv_file() {
    let text = "[virt-viewer]\n\
        secure-attention=Ctrl+Alt+Ins\n\
        delete-this-file=1\n\
        proxy=http://node1.example.com:3128\n\
        type=spice\n\
        tls-port=61000\n\
        host=pvespiceproxy:6a3c0f:101:node1::0f1e2d\n\
        password=0123abcd\n\
        host-subject=OU=PVE Cluster Node,O=Proxmox Virtual Environment,CN=node1.example.com\n\
        ca=-----BEGIN CERTIFICATE-----\\nMIIF\\n-----END CERTIFICATE-----\\n\n\
        title=VM 101 - web\n\
        toggle-fullscreen=Shift+F11\n";
    let vv = parse(text).unwrap();
    assert_eq!(vv.host, "pvespiceproxy:6a3c0f:101:node1::0f1e2d");
    assert_eq!(vv.port, 0);
    assert_eq!(vv.tls_port, Some(61000));
    assert_eq!(vv.password, "0123abcd");
    assert_eq!(vv.proxy.as_deref(), Some("http://node1.example.com:3128"));
    assert_eq!(vv.ca.as_deref(), Some("-----BEGIN CERTIFICATE-----\nMIIF\n-----END CERTIFICATE-----\n"));
    assert_eq!(vv.host_subject.as_deref(), Some("OU=PVE Cluster Node,O=Proxmox Virtual Environment,CN=node1.example.com"));
    assert_eq!(vv.title.as_deref(), Some("VM 101 - web"));
    assert!(vv.delete_after);
}

#[test]
fn libvirt_vv_file() {
    let vv = parse("[virt-viewer]\ntype=spice\nhost=10.0.0.5\nport=5901\npassword=secret\n").unwrap();
    assert_eq!((vv.host.as_str(), vv.port, vv.tls_port), ("10.0.0.5", 5901, None));
    assert!(!vv.delete_after);
}

#[test]
fn vv_file_refusals() {
    assert!(parse("type=spice\nhost=a\nport=1\n").is_err());
    assert!(parse("[virt-viewer]\ntype=vnc\nhost=a\nport=5900\n").unwrap_err().contains("vnc"));
    assert!(parse("[virt-viewer]\ntype=spice\nport=5900\n").is_err());
    assert!(parse("[virt-viewer]\ntype=spice\nhost=a\n").is_err());
    // Keys outside the section do not count.
    assert!(parse("[other]\ntype=spice\nhost=a\nport=1\n[virt-viewer]\n").is_err());
}

fn params() -> SpiceConnectParams {
    SpiceConnectParams { id: "t".into(), host: "vm.example.com".into(), port: 5900, ..Default::default() }
}

#[test]
fn plain_and_tls_configs() {
    let c = connection_config(&params(), "vm.example.com".into(), 5900, false).unwrap();
    assert_eq!((c.port, c.tls_port, c.password), (5900, None, None));

    let p = SpiceConnectParams { tls_port: Some(5901), password: "pw".into(), ca: Some("  ".into()), ..params() };
    let c = connection_config(&p, "vm.example.com".into(), 5900, false).unwrap();
    assert_eq!((c.tls_port, c.password.as_deref(), c.ca_cert), (Some(5901), Some("pw"), None));

    // Through SSH, the leg's local end is the TLS port.
    let c = connection_config(&p, "127.0.0.1".into(), 40000, true).unwrap();
    assert_eq!((c.host.as_str(), c.port, c.tls_port), ("127.0.0.1", 40000, Some(40000)));
}

#[test]
fn proxy_needs_no_ssh_and_a_valid_uri() {
    let p = SpiceConnectParams {
        tls_port: Some(61000),
        proxy: Some("http://node1.example.com:3128".into()),
        host_subject: Some("CN=node1".into()),
        ..params()
    };
    let c = connection_config(&p, p.host.clone(), 0, false).unwrap();
    assert!(c.proxy.is_some());
    assert!(connection_config(&p, "127.0.0.1".into(), 40000, true).is_err());
    let bad = SpiceConnectParams { proxy: Some("ftp://x".into()), ..p };
    assert!(connection_config(&bad, bad.host.clone(), 0, false).is_err());
}

#[test]
fn scancodes_on_the_wire() {
    // `a` down and up; Up arrow (E0 48) down and up.
    assert_eq!(make_scancode(0x1e, false), 0x1e);
    assert_eq!(make_scancode(0x1e, true), 0x9e);
    assert_eq!(make_scancode(0xE048, false), 0x48E0);
    assert_eq!(make_scancode(0xE048, true), 0xC8E0);
}

/// Against a real SPICE server: `SPICE_TEST_ADDR=127.0.0.1:5930 cargo test
/// --lib spice::tests::live -- --ignored --nocapture`, for instance with
/// `qemu-system-x86_64 -vga qxl -spice port=5930,disable-ticketing=on`.
/// Writes what the pump drew to `SPICE_TEST_PNG` when that is set.
#[tokio::test]
#[ignore]
async fn live() {
    let addr = std::env::var("SPICE_TEST_ADDR").expect("SPICE_TEST_ADDR=host:port");
    let (host, port) = addr.rsplit_once(':').unwrap();
    let params = SpiceConnectParams {
        id: "live".into(),
        host: host.into(),
        port: port.parse().unwrap(),
        password: std::env::var("SPICE_TEST_PASSWORD").unwrap_or_default(),
        ..Default::default()
    };
    let config = connection_config(&params, params.host.clone(), params.port, false).unwrap();
    let (run, link) = start(config, None);
    tokio::pin!(run);
    let Link { mut events, input, resize, mouse_mode, cancel } = link;
    let mut pump = Pump::new("live", mouse_mode, input, resize);
    let statuses = std::sync::Mutex::new(Vec::new());
    let status = |s: RdpStatus| statuses.lock().unwrap().push(format!("{s:?}"));
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        tokio::select! {
            ended = &mut run => panic!("session ended: {ended:?}"),
            event = events.recv() => pump.absorb(event.expect("events"), &status).expect("absorb"),
            _ = tokio::time::sleep_until(until) => break,
        }
    }
    // Keys typed, a key by scancode (Enter) and a click go out; what they
    // did on the guest is in the picture.
    if let Ok(text) = std::env::var("SPICE_TEST_TYPE") {
        pump.command(Input::Type(text)).await.unwrap();
    }
    pump.command(Input::Key { scancode: 0x1c, extended: false, down: true }).await.unwrap();
    pump.command(Input::Key { scancode: 0x1c, extended: false, down: false }).await.unwrap();
    pump.command(Input::Mouse { x: 10, y: 10, action: crate::vnc::MouseAction::Down, button: 0, delta: 0 }).await.unwrap();
    pump.command(Input::Mouse { x: 10, y: 10, action: crate::vnc::MouseAction::Up, button: 0, delta: 0 }).await.unwrap();
    let until = Instant::now() + Duration::from_secs(4);
    loop {
        tokio::select! {
            ended = &mut run => panic!("session ended: {ended:?}"),
            event = events.recv() => pump.absorb(event.expect("events"), &status).expect("absorb"),
            _ = tokio::time::sleep_until(until) => break,
        }
    }
    cancel.store(true, Ordering::SeqCst);

    assert!(pump.connected, "never connected; statuses {:?}", statuses.lock().unwrap());
    let surface = pump.mirror.primary_surface().expect("a primary surface");
    let (w, h) = surface.size();
    println!("primary surface {w}x{h}, screen {:?}", pump.screen.size());
    assert_eq!(pump.screen.size(), (w as u16, h as u16));
    assert!(pump.screen.pending());
    let frame = pump.screen.take();
    assert!(frame.len() > (w * h) as usize, "a full frame of {w}x{h} went out ({} bytes)", frame.len());
    if let Ok(path) = std::env::var("SPICE_TEST_PNG") {
        image::save_buffer(path, surface.pixels(), w, h, image::ColorType::Rgba8).unwrap();
    }
}

/// A wrong password fails with a message saying so.
#[tokio::test]
#[ignore]
async fn live_wrong_password() {
    let addr = std::env::var("SPICE_TEST_ADDR").expect("SPICE_TEST_ADDR=host:port");
    let (host, port) = addr.rsplit_once(':').unwrap();
    let params = SpiceConnectParams { id: "live".into(), host: host.into(), port: port.parse().unwrap(), password: "wrong".into(), ..Default::default() };
    let config = connection_config(&params, params.host.clone(), params.port, false).unwrap();
    let (run, _link) = start(config, None);
    let e = tokio::time::timeout(Duration::from_secs(20), run).await.expect("an answer").expect_err("refused");
    let message = connect_error(&e, &params, params.port);
    println!("{e:#} -> {message}");
    assert!(message.starts_with("Wrong SPICE password"), "{message}");
}
