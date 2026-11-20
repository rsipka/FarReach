//! X11 Desktop sessions: an SSH login that starts a whole desktop (GNOME,
//! KDE Plasma, Xfce, MATE, Cinnamon, LXQt, Openbox or a command of the
//! user's own) and shows it in one window here. The window is a nested X
//! server Reach starts for the session alone: Xephyr on Linux (and macOS,
//! when installed), VcXsrv in windowed mode on Windows. On a Mac without
//! Xephyr the desktop goes to XQuartz's own display.
//!
//! The nested server holds nothing but this desktop, so the forwarding is
//! trusted (desktops break under the SECURITY extension) without giving the
//! server a look at any other window. Its cookie stays here, as with any
//! X11 forwarding (see `x11`), and the server ends with the session.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// The desktop a session starts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct X11Desktop {
    /// gnome, kde, xfce, mate, cinnamon, lxqt, openbox or custom.
    pub desktop: String,
    /// The start command, for `custom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// The start commands to try for each desktop, newest first.
fn commands(desktop: &str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match desktop {
        "gnome" => (&["gnome-session"], "GNOME"),
        "kde" => (&["startplasma-x11", "startkde"], "KDE"),
        "xfce" => (&["startxfce4", "xfce4-session"], "XFCE"),
        "mate" => (&["mate-session"], "MATE"),
        "cinnamon" => (&["cinnamon-session", "cinnamon-session-cinnamon"], "X-Cinnamon"),
        "lxqt" => (&["startlxqt"], "LXQt"),
        "openbox" => (&["openbox-session"], "Openbox"),
        _ => return None,
    })
}

/// What a shell script needs quoted inside single quotes.
fn sq(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

impl X11Desktop {
    pub fn check(&self) -> Result<(), String> {
        match self.desktop.as_str() {
            "custom" if self.command.as_deref().is_none_or(|c| c.trim().is_empty()) => Err("X11 desktop: the custom desktop needs a start command".into()),
            "custom" => Ok(()),
            d if commands(d).is_some() => Ok(()),
            d => Err(format!("Unknown X11 desktop: {d}")),
        }
    }

    /// The remote command that starts the desktop on the forwarded display,
    /// with its own D-Bus session, as X11 (never Wayland). It says what is
    /// missing when the server did not forward X11 or lacks the desktop.
    pub fn launcher(&self) -> Result<String, String> {
        self.check()?;
        let mut s = String::from(
            "unset WAYLAND_DISPLAY; export XDG_SESSION_TYPE=x11; \
             if [ -z \"$DISPLAY\" ]; then echo \"Reacher CM: the server did not forward X11. Set X11Forwarding yes in its sshd_config and install xauth.\" >&2; exit 1; fi; \
             dbus() { if command -v dbus-run-session >/dev/null 2>&1; then exec dbus-run-session -- \"$@\"; elif command -v dbus-launch >/dev/null 2>&1; then exec dbus-launch --exit-with-session \"$@\"; else exec \"$@\"; fi; }; ",
        );
        if self.desktop == "custom" {
            let cmd = self.command.as_deref().unwrap_or_default().trim();
            s.push_str(&format!("dbus sh -c {}", sq(cmd)));
        } else {
            let (cmds, name) = commands(&self.desktop).expect("checked");
            s.push_str(&format!("export XDG_CURRENT_DESKTOP={name}; for c in {}; do if command -v \"$c\" >/dev/null 2>&1; then dbus \"$c\"; fi; done; ", cmds.join(" ")));
            s.push_str(&format!("echo \"Reacher CM: {} is not installed on this server.\" >&2; exit 127", cmds[0]));
        }
        Ok(format!("sh -c {}", sq(&s)))
    }
}

/// Makes `opts` an X11 Desktop connection: a session channel that asks for
/// trusted X11 forwarding to a nested X server, started now. Fails when no
/// nested server can be started, since the desktop has nowhere to go.
pub async fn apply(opts: &mut crate::ssh::client::HopOptions, host: &str, title: &str) -> Result<(), String> {
    if opts.session.is_none() {
        let empty = crate::ssh::sshconf::resolve::Resolved::empty(host);
        let mut p = crate::ssh::session_opts::SessionPolicy::from_options(&empty.options, None, false);
        // The terminal only shows the desktop's messages: no ~ escapes.
        p.escape = None;
        opts.session = Some(Arc::new(p));
    }
    let table = opts.forwards.get_or_insert_with(|| crate::ssh::forwarding::ForwardTable::new(Default::default())).clone();
    table.force_x11(crate::ssh::x11::X11Config { display: String::new(), trusted: true, timeout: None, xauth: String::new(), nested: Some(title.to_string()) });
    match table.prepare_x11().await {
        Some(_) => Ok(()),
        None => Err(table.x11_error().unwrap_or_else(|| "X11 desktop: no X server to show it in".into())),
    }
}

/// A nested X server Reach started for one session; ends when dropped.
pub struct Nested {
    pub display: String,
    pub cookie: Vec<u8>,
    pub auth_file: std::path::PathBuf,
    child: std::process::Child,
    /// The authority file's folder, or the Windows job tying the server to
    /// Reach.
    _keep: Box<dyn Send + Sync>,
}

impl std::fmt::Debug for Nested {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Nested").field("display", &self.display).field("pid", &self.child.id()).finish()
    }
}

impl Drop for Nested {
    fn drop(&mut self) {
        // SIGTERM first: an X server killed outright leaves its socket and
        // lock file behind.
        #[cfg(unix)]
        if let Ok(pid) = i32::try_from(self.child.id()) {
            // SAFETY: a plain signal to the child Reach started and has not
            // reaped yet, so the pid is still its.
            unsafe { libc::kill(pid, libc::SIGTERM) };
            for _ in 0..20 {
                if matches!(self.child.try_wait(), Ok(Some(_))) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        tracing::info!("X11 desktop: closed the window on display {}", self.display);
    }
}

impl Nested {
    pub(crate) fn new(display: String, cookie: Vec<u8>, auth_file: std::path::PathBuf, child: std::process::Child, keep: Box<dyn Send + Sync>) -> Nested {
        Nested { display, cookie, auth_file, child, _keep: keep }
    }
}

/// Starts the window for a desktop. `Ok(None)`: none can be started here,
/// but the system's own display (XQuartz on a Mac) takes the desktop.
pub async fn start(title: &str) -> Result<Option<Nested>, String> {
    #[cfg(windows)]
    {
        let _ = title;
        super::xserver::start_nested().await.map(Some)
    }
    #[cfg(unix)]
    {
        unix::start(title).await
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = title;
        Err("X11 desktops are not available on this system".into())
    }
}

#[cfg(unix)]
mod unix {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use super::Nested;

    /// Where Xephyr is: PATH, then where MacPorts and Homebrew put it.
    fn find_xephyr() -> Option<PathBuf> {
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path)
            .chain(["/usr/bin", "/usr/local/bin", "/opt/local/bin", "/opt/homebrew/bin", "/opt/X11/bin"].map(PathBuf::from))
            .map(|d| d.join("Xephyr"))
            .find(|p| p.is_file())
    }

    /// A display number nothing uses: no socket and no lock file.
    fn free_display() -> Option<u16> {
        (10u16..100).find(|n| !Path::new(&format!("/tmp/.X11-unix/X{n}")).exists() && !Path::new(&format!("/tmp/.X{n}-lock")).exists())
    }

    pub async fn start(title: &str) -> Result<Option<Nested>, String> {
        let Some(xephyr) = find_xephyr() else {
            if cfg!(target_os = "macos") && std::env::var("DISPLAY").is_ok_and(|d| !d.is_empty()) {
                tracing::warn!("X11 desktop: Xephyr is not installed; the desktop goes to XQuartz (its full screen mode suits it)");
                return Ok(None);
            }
            return Err(if cfg!(target_os = "macos") {
                "X11 desktop: install XQuartz (xquartz.org), log out and in, and connect again".into()
            } else {
                "X11 desktop: Xephyr is not installed on this computer (Debian/Ubuntu: xserver-xephyr, Fedora: xorg-x11-server-Xephyr, Arch: xorg-server-xephyr)".into()
            });
        };
        let n = free_display().ok_or("X11 desktop: no free X display number")?;
        let cookie: [u8; 16] = rand::random();
        let dir = crate::ssh::x11::PrivateDir::new().map_err(|e| format!("X11 desktop: {e}"))?;
        let auth_file = dir.path().join("auth");
        std::fs::write(&auth_file, crate::ssh::x11::authority(n, &cookie)).map_err(|e| format!("X11 desktop: {e}"))?;
        let mut child = std::process::Command::new(&xephyr)
            .arg(format!(":{n}"))
            .arg("-auth")
            .arg(&auth_file)
            .args(["-nolisten", "tcp", "-resizeable", "-screen", "1280x800", "-title", title])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("X11 desktop: could not start {}: {e}", xephyr.display()))?;
        let socket = PathBuf::from(format!("/tmp/.X11-unix/X{n}"));
        for _ in 0..100 {
            if socket.exists() {
                tracing::info!("X11 desktop: started Xephyr on display :{n}");
                return Ok(Some(Nested::new(format!(":{n}"), cookie.to_vec(), auth_file, child, Box::new(dir))));
            }
            if let Ok(Some(status)) = child.try_wait() {
                return Err(format!("X11 desktop: Xephyr exited ({status}); it needs a desktop session of its own to open its window in"));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let _ = child.kill();
        let _ = child.wait();
        Err("X11 desktop: Xephyr did not start".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(desktop: &str, command: Option<&str>) -> X11Desktop {
        X11Desktop { desktop: desktop.into(), command: command.map(Into::into) }
    }

    #[test]
    fn launchers() {
        let l = d("xfce", None).launcher().unwrap();
        assert!(l.starts_with("sh -c '"), "{l}");
        assert!(l.contains("for c in startxfce4 xfce4-session;"), "{l}");
        assert!(l.contains("XDG_CURRENT_DESKTOP=XFCE"), "{l}");
        assert!(l.contains("unset WAYLAND_DISPLAY"), "{l}");
        for k in ["gnome", "kde", "mate", "cinnamon", "lxqt", "openbox"] {
            assert!(d(k, None).launcher().is_ok(), "{k}");
        }
        assert!(d("windows95", None).launcher().is_err());
        assert!(d("custom", None).launcher().is_err());
        assert!(d("custom", Some("  ")).launcher().is_err());
    }

    #[test]
    fn custom_command_is_quoted() {
        let l = d("custom", Some("echo 'hi'; i3")).launcher().unwrap();
        // The quoting survives a real shell: run the inner script's quoting.
        let out = std::process::Command::new("sh").arg("-c").arg(format!("printf %s {}", sq("echo 'hi'; i3"))).output();
        if let Ok(o) = out {
            assert_eq!(String::from_utf8_lossy(&o.stdout), "echo 'hi'; i3");
        }
        assert!(l.contains("dbus sh -c"), "{l}");
    }

    #[cfg(unix)]
    #[test]
    fn launcher_runs_in_sh() {
        // Without DISPLAY it stops with the forwarding message.
        let l = d("xfce", None).launcher().unwrap();
        let Ok(o) = std::process::Command::new("sh").arg("-c").arg(&l).env_remove("DISPLAY").output() else { return };
        assert_eq!(o.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&o.stderr).contains("X11Forwarding yes"));
    }

    /// A whole desktop through a real SSH server into a real Xephyr.
    /// X11_DESKTOP_TEST_ADDR=127.0.0.1:2222, X11_DESKTOP_TEST_USER,
    /// X11_DESKTOP_TEST_PASSWORD, X11_DESKTOP_TEST_DESKTOP (openbox), and
    /// DISPLAY for Xephyr's own window (an Xvfb will do).
    /// X11_DESKTOP_TEST_PNG: a screenshot of the nested display.
    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "needs an SSH server with a desktop, Xephyr and a display"]
    async fn live() {
        use russh::ChannelMsg;
        let addr = std::env::var("X11_DESKTOP_TEST_ADDR").unwrap();
        let (host, port) = addr.rsplit_once(':').unwrap();
        let port: u16 = port.parse().unwrap();
        let user = std::env::var("X11_DESKTOP_TEST_USER").unwrap();
        let desktop = d(&std::env::var("X11_DESKTOP_TEST_DESKTOP").unwrap_or_else(|_| "openbox".into()), None);
        let kh = std::env::temp_dir().join(format!("reach-x11d-kh-{}", std::process::id()));
        let o = crate::ssh::sshconf::session::SshOptions {
            lines: vec![format!("UserKnownHostsFile {}", kh.display()), "StrictHostKeyChecking accept-new".into()],
            ..Default::default()
        };
        let mut opts: crate::ssh::client::HopOptions = crate::ssh::sshconf::session::plan_for(Some(&o), host, port, &user, false).into();
        apply(&mut opts, host, "Reacher CM test").await.unwrap();
        let table = opts.forwards.clone().unwrap();
        let x11 = table.prepared_x11().unwrap();
        let (display, auth) = x11.nested().map(|(d, a)| (d.to_string(), a.to_path_buf())).unwrap();

        let stream = crate::ssh::sshconf::net::connect(host, port, &opts.socket, false).await.unwrap();
        let handler = crate::ssh::client::SshClientHandler::new(host, port, None).with_hostkeys(opts.hostkeys.clone()).with_forwards(opts.forwards.clone());
        let mut handle = russh::client::connect_stream(opts.config.clone(), stream, handler).await.unwrap();
        let login = crate::ssh::client::AuthParams { key: None, password: std::env::var("X11_DESKTOP_TEST_PASSWORD").ok(), allow_agent: false };
        crate::ssh::client::login(&mut handle, &user, &login, &opts, None, host, port).await.unwrap().into_result().unwrap();
        let mut ch = handle.channel_open_session().await.unwrap();
        crate::ssh::session_opts::setup_channel(&ch, opts.session.as_ref().unwrap(), 80, 24, Some(&desktop.launcher().unwrap()), Some(&x11)).await.unwrap();

        // The window manager takes the nested display within a few seconds.
        let mut out = String::new();
        let mut managed = false;
        for _ in 0..40 {
            if let Ok(Some(msg)) = tokio::time::timeout(std::time::Duration::from_millis(250), ch.wait()).await {
                match msg {
                    ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => out.push_str(&String::from_utf8_lossy(&data)),
                    ChannelMsg::ExitStatus { exit_status } => panic!("the desktop exited ({exit_status}): {out}"),
                    _ => {}
                }
            }
            let p = std::process::Command::new("xprop").args(["-display", &display, "-root", "_NET_SUPPORTING_WM_CHECK"]).env("XAUTHORITY", &auth).output().unwrap();
            if String::from_utf8_lossy(&p.stdout).contains("window id") {
                managed = true;
                break;
            }
        }
        println!("{out}");
        assert!(managed, "no window manager on the nested display: {out}");
        if let Ok(png) = std::env::var("X11_DESKTOP_TEST_PNG") {
            let _ = std::process::Command::new("sh").arg("-c").arg(format!("xwd -root -display {display} | convert xwd:- {png}")).env("XAUTHORITY", &auth).status();
        }
        // Without the cookie the nested display lets nobody in.
        let p = std::process::Command::new("xprop").args(["-display", &display, "-root"]).env("XAUTHORITY", "/nonexistent").output().unwrap();
        assert!(!p.status.success(), "the nested display took a client without its cookie");
        drop(ch);
        drop(handle);
        drop(x11);
        drop(opts);
        drop(table);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(!std::path::Path::new(&format!("/tmp/.X11-unix/X{}", display.trim_start_matches(':'))).exists(), "Xephyr outlived the session");
        std::fs::remove_file(&kh).ok();
    }
}
