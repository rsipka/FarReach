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
    /// Ask first: the server's tmux sessions are listed, and the user picks
    /// one, a new one (`name`), or no tmux.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ask: bool,
    /// Turn tmux's mouse mode on for this session, so the wheel scrolls
    /// tmux's history (the session's own option; .tmux.conf stays as it is).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mouse: bool,
}

/// A session's own tmux choice, as saved with it. `mode` is `off`,
/// `attach` (attach or create), `ask` (pick from the server's sessions) or
/// `inherit` (as its folder says, with a name of its own); kept as text so a mode a newer version adds
/// does not make the session unreadable here. Unset on the session: its
/// folder's choice, else off.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TmuxSetting {
    pub mode: String,
    /// The tmux session's name; unset: one made from the session's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// tmux's mouse mode for this session (see `TmuxStart::mouse`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mouse: bool,
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

/// One tmux session on the server, as the picker lists it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub name: String,
    pub windows: u32,
    /// Clients attached now.
    pub attached: u32,
    /// Last activity, Unix seconds.
    pub activity: u64,
}

/// Lists the server's tmux sessions, one per line, fields split by `:`,
/// which tmux never allows in a session name (a tab would come out as `_`).
pub const LIST_COMMAND: &str =
    "tmux list-sessions -F '#{session_name}:#{session_windows}:#{session_attached}:#{session_activity}:#{session_group}' 2>/dev/null";

/// The sessions in `tmux list-sessions` output (see `LIST_COMMAND`), most
/// recently used first. Sessions another session was grouped with (a
/// second tab's view, `NAME-1`) are not listed: their group is.
pub fn parse_list(out: &str) -> Vec<Listed> {
    let mut list: Vec<Listed> = out
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split(':').collect();
            if f.len() < 4 || f[0].is_empty() {
                return None;
            }
            let group = f.get(4).copied().unwrap_or("");
            if !group.is_empty() && group != f[0] {
                return None;
            }
            Some(Listed {
                name: f[0].to_string(),
                windows: f[1].parse().unwrap_or(0),
                attached: f[2].parse().unwrap_or(0),
                activity: f[3].parse().unwrap_or(0),
            })
        })
        .collect();
    list.sort_by_key(|l| std::cmp::Reverse(l.activity));
    list
}

/// What the picker answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Attach to this tmux session, or create it.
    Session(String),
    /// The session's own shell, no tmux.
    NoTmux,
}

/// The picker's answer: a session name, or `NO_TMUX`.
pub const NO_TMUX_ANSWER: &str = "\u{1}none";

pub fn read_choice(answer: &str, default_name: &str) -> Result<Choice, String> {
    if answer == NO_TMUX_ANSWER {
        return Ok(Choice::NoTmux);
    }
    let name = if answer.trim().is_empty() { default_name } else { answer.trim() };
    if valid_name(name) {
        Ok(Choice::Session(name.to_string()))
    } else {
        // Someone else's session with a name the script cannot hold.
        Err(format!("The tmux session '{name}' has a name this app cannot attach to; only letters, digits, '-' and '_' work"))
    }
}

/// A tmux session to be picked while the connection opens: the picker
/// needs the logged-in connection to list what the server runs.
#[derive(Clone)]
pub struct Ask {
    /// The connection being opened; the answer is kept under it for the
    /// window (`take_chosen`).
    pub id: String,
    pub start: TmuxStart,
    /// The session's login shell.
    pub shell: Option<String>,
    pub app: tauri::AppHandle,
    pub host: String,
    pub port: u16,
}

impl std::fmt::Debug for Ask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ask").field("id", &self.id).field("start", &self.start).finish()
    }
}

fn chosen() -> &'static std::sync::Mutex<std::collections::HashMap<String, Option<String>>> {
    static C: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, Option<String>>>> = std::sync::OnceLock::new();
    C.get_or_init(Default::default)
}

/// What the picker chose for connection `id`, once: `Some(Some(name))` a
/// tmux session, `Some(None)` no tmux, `None` nothing was asked.
pub fn take_chosen(id: &str) -> Option<Option<String>> {
    chosen().lock().unwrap().remove(id)
}

impl Ask {
    /// List the server's sessions, ask, and give the command the session
    /// channel runs (`None`: the login shell as usual).
    pub async fn resolve(&self, handle: &russh::client::Handle<super::client::SshClientHandler>) -> Result<Option<String>, super::client::SshError> {
        use super::client::SshError;
        let listed = parse_list(&list_on(handle).await.unwrap_or_default());
        let fields = listed
            .iter()
            .map(|l| super::prompt::Field {
                text: format!("{}\t{}\t{}\t{}", l.name, l.windows, l.attached, l.activity),
                echo: true,
            })
            .collect();
        let answer = super::prompt::ask(
            Some(&self.app),
            &self.host,
            self.port,
            super::prompt::Kind::Tmux,
            &self.start.name,
            "",
            fields,
        )
        .await
        .ok_or_else(|| SshError::ConnectionFailed("Connection cancelled".into()))?;
        let choice = read_choice(answer.first().map(String::as_str).unwrap_or(""), &self.start.name).map_err(SshError::ConnectionFailed)?;
        let (record, command) = match choice {
            Choice::NoTmux => (None, None),
            Choice::Session(name) => {
                let start = TmuxStart { name: name.clone(), ask: false, mouse: self.start.mouse };
                let cmd = start_command(&start, self.shell.as_deref()).map_err(SshError::ConnectionFailed)?;
                (Some(name), Some(cmd))
            }
        };
        chosen().lock().unwrap().insert(self.id.clone(), record);
        Ok(command.or_else(|| self.shell.clone()))
    }
}

/// Runs `LIST_COMMAND` on a fresh channel of `handle`.
async fn list_on(handle: &russh::client::Handle<super::client::SshClientHandler>) -> Option<String> {
    let mut channel = handle.channel_open_session().await.ok()?;
    channel.exec(true, LIST_COMMAND).await.ok()?;
    let mut out = Vec::new();
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(10), channel.wait()).await {
            Ok(Some(russh::ChannelMsg::Data { data })) => out.extend_from_slice(&data),
            Ok(Some(russh::ChannelMsg::Eof)) | Ok(None) | Err(_) => break,
            _ => {}
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// The message printed when the server has no tmux.
const NO_TMUX: &str = "tmux was not found on this server, so a normal shell was opened.";

/// The command the channel runs for `start`, given the session's login
/// shell. Run by the account's login shell as `$SHELL -c`, so it is one
/// `sh -c '…'`, valid in sh, bash, zsh, fish and csh alike.
pub fn start_command(start: &TmuxStart, shell: Option<&str>) -> Result<String, String> {
    let script = start_script(start, shell)?;
    // Single quotes hold the script as it is in every shell above; it has
    // none of its own.
    debug_assert!(!script.contains('\''));
    // No `exec` of its own: the channel adds it to every login shell
    // setting (client.rs, session_opts.rs), and `exec exec` fails in bash.
    Ok(format!("sh -c '{script}'"))
}

/// The script itself, for `sh -c` (a WSL tab runs it with `wsl.exe -e`).
pub fn start_script(start: &TmuxStart, shell: Option<&str>) -> Result<String, String> {
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
            "exec tmux $F new-session -t ={n} \\; set-option destroy-unattached on{mouse}; ",
            "fi; ",
            "exec tmux $F new-session -A -s {n}{window}{mouse}; ",
            "fi; ",
            "printf \"\\033[33m%s\\033[0m\\r\\n\" \"{msg}\"; ",
            "{fallback}"
        ),
        n = n,
        window = window_arg,
        msg = NO_TMUX,
        fallback = fallback,
        mouse = if start.mouse { " \\; set-option mouse on" } else { "" },
    );
    Ok(script)
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
        TmuxStart { name: name.to_string(), ask: false, mouse: false }
    }

    #[test]
    fn mouse_mode_is_set_on_the_session_only() {
        let c = start_command(&TmuxStart { name: "w".into(), ask: false, mouse: true }, None).unwrap();
        assert!(c.contains("new-session -A -s w \\; set-option mouse on;"), "{c}");
        assert!(c.contains("destroy-unattached on \\; set-option mouse on;"), "{c}");
        assert!(!c.contains("-g"), "{c}");
    }

    #[test]
    fn the_list_is_read_newest_first_without_grouped_views() {
        let out = "old:1:0:100:\nw:3:1:300:w\nw-1:3:1:400:w\nnew:2:0:200:\nbroken\n";
        let names: Vec<String> = parse_list(out).into_iter().map(|l| l.name).collect();
        assert_eq!(names, ["w", "new", "old"]);
        assert_eq!(parse_list(out)[0], Listed { name: "w".into(), windows: 3, attached: 1, activity: 300 });
    }

    #[test]
    fn the_list_command_splits_on_colons() {
        assert!(LIST_COMMAND.contains("#{session_name}:#{session_windows}"));
        assert_eq!(LIST_COMMAND.matches('\'').count(), 2);
    }

    #[test]
    fn a_choice_is_a_name_the_default_or_no_tmux() {
        assert_eq!(read_choice("main", "reacher-w"), Ok(Choice::Session("main".into())));
        assert_eq!(read_choice("", "reacher-w"), Ok(Choice::Session("reacher-w".into())));
        assert_eq!(read_choice(NO_TMUX_ANSWER, "reacher-w"), Ok(Choice::NoTmux));
        assert!(read_choice("my session", "reacher-w").is_err());
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
        assert!(c.starts_with("sh -c '"), "{c}");
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
    /// Not on Android, whose shell is not at /bin/sh.
    #[cfg(all(unix, not(target_os = "android")))]
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
                    "#!/bin/sh\ncase \"$*\" in '-TRGB -V') exit 0;; list-clients*) {clients}; exit 0;; esac\necho \"$@\" >> {}\n",
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
        // As the channel sends it: every login shell setting gets an `exec`.
        let out = Command::new("sh")
            .arg("-c")
            .arg(format!("exec {cmd}"))
            .env("PATH", &dir)
            .output()
            .unwrap();
        let logged = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        format!("{}{}", logged, String::from_utf8_lossy(&out.stdout))
    }

    #[cfg(all(unix, not(target_os = "android")))]
    #[test]
    fn the_script_attaches_or_creates() {
        assert_eq!(run_with_fake_tmux(false, true).trim(), "-TRGB new-session -A -s w say -l");
    }

    #[cfg(all(unix, not(target_os = "android")))]
    #[test]
    fn the_script_joins_as_a_group_when_someone_is_attached() {
        assert_eq!(run_with_fake_tmux(true, true).trim(), "-TRGB new-session -t =w ; set-option destroy-unattached on");
    }

    #[cfg(all(unix, not(target_os = "android")))]
    #[test]
    fn without_tmux_the_script_says_so_and_runs_the_shell() {
        let out = run_with_fake_tmux(false, false);
        assert!(out.contains(NO_TMUX), "{out}");
        // `say -l` is the stand-in login shell: it prints its arguments.
        assert!(out.trim_end().ends_with("-l"), "{out}");
    }
}
