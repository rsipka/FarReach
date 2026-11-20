//! The tap against lrzsz's own `sz` and `rz`, over pipes, the way a session
//! loop drives it. Skipped where lrzsz is not installed.

use super::*;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

struct Run {
    events: Arc<Mutex<Vec<ZmodemEvent>>>,
    shown: Vec<u8>,
}

/// Drive `tap` against `child` until it exits; `answer` is called on each
/// new event and may decide for the user. Reads are cut to `chunk` bytes.
fn drive(id: &str, mut child: Child, chunk: usize, mut answer: impl FnMut(&ZmodemEvent) -> Option<Decision>) -> Run {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let mut tap = Tap::new(id, WINDOW_NETWORK, Box::new(move |e| sink.lock().unwrap().push(e)));
    let mut stdout = child.stdout.take().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    for piece in buf[..n].chunks(chunk) {
                        if tx.send(piece.to_vec()).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });
    let mut shown = Vec::new();
    let mut seen = 0;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        assert!(Instant::now() < deadline, "transfer hung");
        let out = match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(data) => tap.input(&data),
            Err(mpsc::RecvTimeoutError::Timeout) => tap.poll(),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                tap.poll()
            }
        };
        shown.extend(out.display);
        if !out.wire.is_empty() && stdin.write_all(&out.wire).is_err() {
            // The program ended.
        }
        let _ = stdin.flush();
        let new: Vec<ZmodemEvent> = events.lock().unwrap()[seen..].to_vec();
        seen += new.len();
        for e in new {
            if let Some(d) = answer(&e) {
                decide(id, d).unwrap();
            }
        }
    }
    let _ = child.wait();
    // Let the tap see the end of its drain.
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(120));
        shown.extend(tap.poll().display);
    }
    Run { events, shown }
}

fn pattern(len: usize, seed: u32) -> Vec<u8> {
    // Every byte value, the ZMODEM specials among them, in a run that does
    // not repeat on 1 KiB boundaries.
    let mut x = seed;
    (0..len)
        .map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (x >> 16) as u8
        })
        .collect()
}

fn spawn(cmd: &str, args: &[&str], dir: &std::path::Path) -> Child {
    Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

fn last(run: &Run) -> ZmodemEvent {
    run.events.lock().unwrap().last().cloned().unwrap()
}

#[test]
fn download_from_sz() {
    if !have("sz") {
        return;
    }
    for (chunk, size) in [(16 * 1024, 300_000), (7, 5000), (1, 1500)] {
        let remote = tempfile::tempdir().unwrap();
        let here = tempfile::tempdir().unwrap();
        let data = pattern(size, size as u32);
        std::fs::write(remote.path().join("fw.bin"), &data).unwrap();
        std::fs::write(remote.path().join("empty.txt"), b"").unwrap();
        // A file of the same name already here is kept.
        std::fs::write(here.path().join("fw.bin"), b"old").unwrap();
        let folder = here.path().to_path_buf();
        let child = spawn("sz", &["fw.bin", "empty.txt"], remote.path());
        let run = drive(&format!("dl-{chunk}"), child, chunk, |e| match e {
            ZmodemEvent::Offer { direction: Direction::Download } => Some(Decision::Download(folder.clone())),
            _ => None,
        });
        match last(&run) {
            ZmodemEvent::Finished { files, .. } => assert_eq!(files, vec!["fw (1).bin", "empty.txt"]),
            e => panic!("chunk {chunk}: {e:?}"),
        }
        assert_eq!(std::fs::read(here.path().join("fw (1).bin")).unwrap(), data, "chunk {chunk}");
        assert_eq!(std::fs::read(here.path().join("fw.bin")).unwrap(), b"old");
        assert_eq!(std::fs::read(here.path().join("empty.txt")).unwrap(), b"");
        // Only sz's own "rz\r" before the header shows, and no "OO" after.
        assert!(!run.shown.contains(&0x18), "chunk {chunk}: {:?}", String::from_utf8_lossy(&run.shown));
        assert!(!run.shown.ends_with(b"OO"), "chunk {chunk}");
    }
}

#[test]
fn upload_to_rz() {
    if !have("rz") {
        return;
    }
    let remote = tempfile::tempdir().unwrap();
    let here = tempfile::tempdir().unwrap();
    let a = pattern(200_000, 1);
    let b = pattern(1, 2);
    std::fs::write(here.path().join("config.txt"), &a).unwrap();
    std::fs::write(here.path().join("one.bin"), &b).unwrap();
    let files = vec![here.path().join("config.txt"), here.path().join("one.bin")];
    let child = spawn("rz", &["-y"], remote.path());
    let run = drive("ul", child, 4096, |e| match e {
        ZmodemEvent::Offer { direction: Direction::Upload } => Some(Decision::Upload(files.clone())),
        _ => None,
    });
    match last(&run) {
        ZmodemEvent::Finished { files, folder, .. } => {
            assert_eq!(files, vec!["config.txt", "one.bin"]);
            assert!(folder.is_none());
        }
        e => panic!("{e:?}"),
    }
    assert_eq!(std::fs::read(remote.path().join("config.txt")).unwrap(), a);
    assert_eq!(std::fs::read(remote.path().join("one.bin")).unwrap(), b);
    let progress = run.events.lock().unwrap().iter().filter(|e| matches!(e, ZmodemEvent::Progress { .. })).count();
    assert!(progress > 0);
}

#[test]
fn declining_an_offer_stops_sz() {
    if !have("sz") {
        return;
    }
    let remote = tempfile::tempdir().unwrap();
    std::fs::write(remote.path().join("x.bin"), pattern(10_000, 3)).unwrap();
    let child = spawn("sz", &["x.bin"], remote.path());
    let started = Instant::now();
    let run = drive("decline", child, 4096, |e| match e {
        ZmodemEvent::Offer { .. } => Some(Decision::Cancel),
        _ => None,
    });
    assert!(matches!(last(&run), ZmodemEvent::Cancelled { .. }));
    assert!(started.elapsed() < Duration::from_secs(15), "sz did not stop");
}

#[test]
fn cancelling_midway_removes_the_partial_file() {
    if !have("sz") {
        return;
    }
    let remote = tempfile::tempdir().unwrap();
    let here = tempfile::tempdir().unwrap();
    std::fs::write(remote.path().join("big.bin"), pattern(5_000_000, 4)).unwrap();
    let folder = here.path().to_path_buf();
    let child = spawn("sz", &["big.bin"], remote.path());
    let id = "cancel";
    // Small reads keep the transfer slow enough to stop halfway.
    let run = drive(id, child, 256, |e| match e {
        ZmodemEvent::Offer { .. } => Some(Decision::Download(folder.clone())),
        ZmodemEvent::Progress { .. } => Some(Decision::Cancel),
        _ => None,
    });
    assert!(matches!(last(&run), ZmodemEvent::Cancelled { .. }), "{:?}", run.events.lock().unwrap());
    assert_eq!(std::fs::read_dir(here.path()).unwrap().count(), 0);
}

#[test]
fn keys_are_held_while_busy_and_ctrl_c_cancels() {
    let mut tap = Tap::new("keys", WINDOW_SERIAL, Box::new(|_| {}));
    assert!(!intercept_input("keys", b"ls\r"));
    let out = tap.input(b"$ sz x\r\nrz\r**\x18B00000000000000\r\x8a\x11");
    assert_eq!(out.display, b"$ sz x\r\nrz\r*");
    assert!(tap.busy());
    assert!(intercept_input("keys", b"a"));
    assert!(intercept_input("keys", b"\x03"));
    let out = tap.poll();
    assert_eq!(out.wire, CANCEL);
    drop(tap);
    assert!(!intercept_input("keys", b"a"));
}

#[test]
fn names_from_the_remote_are_made_safe() {
    use transfer::safe_name;
    assert_eq!(safe_name("../../etc/passwd"), "passwd");
    assert_eq!(safe_name("C:\\x\\boot.ini"), "boot.ini");
    assert_eq!(safe_name(".."), "download");
    assert_eq!(safe_name(".bashrc"), "bashrc");
    assert_eq!(safe_name("a\x1b[31mb"), "a_[31mb");
    assert_eq!(safe_name("con.txt"), "_con.txt");
    assert_eq!(safe_name("what?.txt"), "what_.txt");
}
