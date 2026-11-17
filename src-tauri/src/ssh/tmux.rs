//! tmux on the server: an SSH session can start inside a named tmux
//! session, so a dropped connection or a closed app loses nothing and the
//! next connect lands where the last one left off.
//!
//! The session is started as the channel's command, the way the login
//! shell field is, through a small `sh -c` script:
//!
//! - no tmux on the server: the login shell, with one line saying so;
//! - somebody already attached (another tab, another computer): a new
//!   session grouped with it, so each sees its own window, removed again
//!   when it is left;
//! - otherwise `new-session -A`: attach, or create it.
//!
//! Only names made of letters, digits, `-` and `_` are accepted, and a
//! login shell is passed on only when it is made of plain words, so
//! nothing from a session can break out of the script.

use serde::{Deserialize, Serialize};

/// The longest name accepted. tmux has no limit, but a status line does.
const MAX_NAME: usize = 64;

/// What a connect asks of tmux.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TmuxStart {
    /// The tmux session to attach to or create.
    pub name: String,
}

/// A session's own tmux choice, as saved with it. `mode` is `off`,
/// `attach` (attach or create) or `inherit` (as its folder says, with a
/// name of its own); kept as text so a mode a newer version adds
/// does not make the session unreadable here. Unset on the session: its
/// folder's choice, else off.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TmuxSetting {
    pub mode: String,
    /// The tmux session's name; unset: one made from the session's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A name tmux takes as it is, which the script can hold unquoted.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && !name.starts_with('-')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A login shell value the script can hold unquoted: words of letters,
/// digits and `-_./=+`. Anything else is left out (the window then runs
/// tmux's own default shell).
fn plain_shell(shell: Option<&str>) -> Option<String> {
    let s = shell.map(str::trim).filter(|s| !s.is_empty())?;
    let ok = s.bytes().all(|b| b.is_ascii_alphanumeric() || b" -_./=+".contains(&b));
    if !ok {
        tracing::warn!("tmux: the login shell '{s}' has characters the start script does not pass on; tmux uses its default shell");
        return None;
    }
    // As for a plain login shell: a bare program runs as a login shell.
    Some(if s.split_whitespace().nth(1).is_some() { s.to_string() } else { format!("{s} -l") })
}

/// The message printed when the server has no tmux.
const NO_TMUX: &str = "tmux was not found on this server, so a normal shell was opened.";

/// The command the channel runs for `start`, given the session's login
/// shell. Run by the account's login shell as `$SHELL -c`, so it is one
/// `exec sh -c '…'`, valid in sh, bash, zsh, fish and csh alike.
pub fn start_command(start: &TmuxStart, shell: Option<&str>) -> Result<String, String> {
    let n = start.name.trim();
    if !valid_name(n) {
        return Err(format!(
            "The tmux session name '{n}' can only use letters, digits, '-' and '_' (at most {MAX_NAME})"
        ));
    }
    let window = plain_shell(shell);
    let window_arg = window.as_deref().map(|w| format!(" {w}")).unwrap_or_default();
    let fallback = window.as_deref().map(|w| format!("exec {w}")).unwrap_or_else(|| "exec \"${SHELL:-/bin/sh}\" -l".to_string());
    let script = format!(
        concat!(
            "if command -v tmux >/dev/null 2>&1; then ",
            // Truecolor, on tmux 3.2 and later; older ones do not know -T.
            "F=; tmux -TRGB -V >/dev/null 2>&1 && F=-TRGB; ",
            "if [ -n \"$(tmux list-clients -t ={n} 2>/dev/null)\" ]; then ",
            "exec tmux $F new-session -t ={n} \\; set-option destroy-unattached on; ",
            "fi; ",
            "exec tmux $F new-session -A -s {n}{window}; ",
            "fi; ",
            "printf \"\\033[33m%s\\033[0m\\r\\n\" \"{msg}\"; ",
            "{fallback}"
        ),
        n = n,
        window = window_arg,
        msg = NO_TMUX,
        fallback = fallback,
    );
    // Single quotes hold the script as it is in every shell above; it has
    // none of its own.
    debug_assert!(!script.contains('\''));
    Ok(format!("exec sh -c '{script}'"))
}

/// Ends the tmux session `name` and every session grouped with it (the
/// ones other tabs joined as), which keep its windows alive otherwise.
pub fn kill_command(name: &str) -> Result<String, String> {
    if !valid_name(name) {
        return Err(format!("'{name}' is not a tmux session name this app uses"));
    }
    Ok(format!(
        "sh -c 'for s in $(tmux list-sessions -F \"#{{session_group}}:#{{session_name}}\" 2>/dev/null | sed -n \"s/^{name}://p\"); do tmux kill-session -t \"=$s\"; done; tmux kill-session -t ={name} 2>/dev/null; exit 0'"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(name: &str) -> TmuxStart {
        TmuxStart { name: name.to_string() }
    }

    #[test]
    fn names_are_plain_words() {
        assert!(valid_name("reacher-web-01"));
        assert!(valid_name("main_2"));
        for bad in ["", "-x", "a b", "a;b", "a'b", "a$b", "a:b", "a.b", "ä", &"x".repeat(65)] {
            assert!(!valid_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_bad_name_is_refused() {
        assert!(start_command(&start("x'; rm -rf ~"), None).is_err());
        assert!(kill_command("a b").is_err());
    }

    #[test]
    fn the_command_is_one_quoted_script() {
        let c = start_command(&start("reacher-web"), None).unwrap();
        assert!(c.starts_with("exec sh -c '"), "{c}");
        assert!(c.ends_with('\''));
        assert_eq!(c.matches('\'').count(), 2, "{c}");
        assert!(c.contains("new-session -A -s reacher-web;"), "{c}");
        assert!(c.contains("new-session -t =reacher-web \\; set-option destroy-unattached on"), "{c}");
        assert!(c.contains("exec \"${SHELL:-/bin/sh}\" -l"), "{c}");
    }

    #[test]
    fn the_login_shell_runs_in_the_window_and_as_the_fallback() {
        let c = start_command(&start("w"), Some("fish")).unwrap();
        assert!(c.contains("new-session -A -s w fish -l;"), "{c}");
        assert!(c.ends_with("exec fish -l'"), "{c}");
        let c = start_command(&start("w"), Some("zsh -i")).unwrap();
        assert!(c.contains("new-session -A -s w zsh -i;"), "{c}");
    }

    #[test]
    fn a_login_shell_with_quotes_is_left_out() {
        let c = start_command(&start("w"), Some("bash -c 'echo hi'")).unwrap();
        assert!(c.contains("new-session -A -s w;"), "{c}");
        assert_eq!(c.matches('\'').count(), 2, "{c}");
    }

    /// Runs the script with a fake tmux on PATH: what it was asked to do.
    #[cfg(unix)]
    fn run_with_fake_tmux(attached: bool, have_tmux: bool) -> String {
        use std::process::Command;
        let dir = std::env::temp_dir().join(format!("reacher-tmux-test-{}-{attached}-{have_tmux}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("log");
        if have_tmux {
            let fake = dir.join("tmux");
            let clients = if attached { "echo /dev/pts/1: w" } else { "true" };
            std::fs::write(
                &fake,
                format!(
                    "#!/bin/sh\ncase \"$1\" in -TRGB) exit 0;; list-clients) {clients}; exit 0;; esac\necho \"$@\" >> {}\n",
                    log.display()
                ),
            )
            .unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // Only these on PATH, so a real tmux on this machine is never found.
        std::os::unix::fs::symlink("/bin/sh", dir.join("sh")).unwrap();
        let shim = dir.join("say");
        std::fs::write(&shim, "#!/bin/sh\necho \"$@\"\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let cmd = start_command(&start("w"), Some("say")).unwrap();
        let out = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .env("PATH", &dir)
            .output()
            .unwrap();
        let logged = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        format!("{}{}", logged, String::from_utf8_lossy(&out.stdout))
    }

    #[cfg(unix)]
    #[test]
    fn the_script_attaches_or_creates() {
        assert_eq!(run_with_fake_tmux(false, true).trim(), "-TRGB new-session -A -s w say -l");
    }

    #[cfg(unix)]
    #[test]
    fn the_script_joins_as_a_group_when_someone_is_attached() {
        assert_eq!(run_with_fake_tmux(true, true).trim(), "-TRGB new-session -t =w ; set-option destroy-unattached on");
    }

    #[cfg(unix)]
    #[test]
    fn without_tmux_the_script_says_so_and_runs_the_shell() {
        let out = run_with_fake_tmux(false, false);
        assert!(out.contains(NO_TMUX), "{out}");
        // `say -l` is the stand-in login shell: it prints its arguments.
        assert!(out.trim_end().ends_with("-l"), "{out}");
    }
}
