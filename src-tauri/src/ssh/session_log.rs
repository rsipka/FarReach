//! Writing an SSH session to a text file, the way PuTTY's Logging panel does.
//!
//! The behaviour follows PuTTY (`logging.c`, `terminal.c`) rather than
//! anything new:
//! - "Printable output" keeps what the terminal prints plus CR, LF and tab,
//!   and drops control sequences. PuTTY gets that from its terminal; here the
//!   output goes through `vte`, the DEC VT parser (Paul Williams' state
//!   machine) that Alacritty uses, so a sequence split across two packets is
//!   still recognised.
//! - "All session output" keeps everything that was sent to the terminal.
//! - The file name takes `&Y &M &D &T &H &P` (case-insensitive) and `&&`,
//!   and whatever they insert has characters that are not allowed in a file
//!   name replaced by `.`, as PuTTY does, so a host such as an IPv6 address
//!   can never add a path separator or a drive colon.
//! - An existing file is appended to unless overwrite is chosen, the header
//!   line is optional, and the data is flushed as it arrives.
//!
//! PuTTY's "SSH packets" modes are left out on purpose: its own manual warns
//! they can write the login password into the file.
//!
//! Only output is logged, never keystrokes, so a password typed at a `sudo`
//! prompt (which the server does not echo) is not in the file. A new file is
//! readable by its owner only on Linux, macOS and Android.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LogMode {
    /// Printable text, CR, LF and tab; control sequences dropped.
    Printable,
    /// Everything that was sent to the terminal.
    All,
}

/// Settings → Terminal → Session logging, as sent with `ssh_connect`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLogConfig {
    pub mode: LogMode,
    /// The folder; empty means [`default_dir`].
    #[serde(default)]
    pub folder: String,
    /// The file name, with PuTTY's `&` placeholders.
    pub name: String,
    /// Add to an existing file (the default) instead of replacing it.
    #[serde(default = "yes")]
    pub append: bool,
    /// Start with PuTTY's date line.
    #[serde(default = "yes")]
    pub header: bool,
}

fn yes() -> bool {
    true
}

pub const DEFAULT_NAME: &str = "&H-&Y&M&D-&T.log";

/// Where logs go when no folder is set: `Documents/Reach Logs` on a desktop,
/// the app's own storage on Android (which has no shared Documents folder an
/// app may write to without asking).
pub fn default_dir() -> PathBuf {
    #[cfg(not(mobile))]
    if let Some(docs) = dirs::document_dir() {
        return docs.join("Reach Logs");
    }
    crate::app_data_dir().join("logs")
}

/// PuTTY's `filename_char_sanitise`, with the Windows set used everywhere so
/// a log name made on one system is valid on the others; control characters
/// go too.
fn sanitise(c: char) -> char {
    if "<>:\"/\\|?*".contains(c) || c.is_control() {
        '.'
    } else {
        c
    }
}

/// PuTTY's `xlatlognam`: expands the placeholders in `name`.
pub fn expand_name(name: &str, host: &str, port: u16, now: &chrono::DateTime<chrono::Local>) -> String {
    let mut out = String::new();
    let mut chars = name.chars();
    while let Some(c) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }
        let Some(k) = chars.next() else {
            // A trailing `&` expands to nothing in PuTTY.
            break;
        };
        let inserted = match k.to_ascii_lowercase() {
            'y' => now.format("%Y").to_string(),
            'm' => now.format("%m").to_string(),
            'd' => now.format("%d").to_string(),
            't' => now.format("%H%M%S").to_string(),
            'h' => host.to_string(),
            'p' => port.to_string(),
            '&' => "&".to_string(),
            other => format!("&{}", other),
        };
        out.extend(inserted.chars().map(sanitise));
    }
    out
}

/// The full path for a session: the folder (or the default) joined with the
/// expanded name. A name may contain sub-folders, but not `..`, and the
/// result must name a file.
pub fn resolve_path(cfg: &SessionLogConfig, host: &str, port: u16, now: &chrono::DateTime<chrono::Local>) -> io::Result<PathBuf> {
    let folder = cfg.folder.trim();
    let base = if folder.is_empty() { default_dir() } else { PathBuf::from(folder) };
    if !base.is_absolute() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "the log folder must be a full path"));
    }
    let name = cfg.name.trim();
    let name = if name.is_empty() { DEFAULT_NAME } else { name };
    let rel = PathBuf::from(expand_name(name, host, port, now));
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "the log file name may not contain '..' or a full path"));
    }
    let path = base.join(&rel);
    if path.file_name().is_none() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "the log file name is empty"));
    }
    Ok(path)
}

fn create_dirs(dir: &Path) -> io::Result<()> {
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(dir)
}

fn open_file(path: &Path, append: bool) -> io::Result<File> {
    let mut o = OpenOptions::new();
    o.create(true);
    if append {
        o.append(true);
    } else {
        o.write(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Applies only when the file is created; an existing file keeps the
        // permissions its owner gave it.
        o.mode(0o600);
    }
    o.open(path)
}

/// PuTTY's header line, with Reach in place of PuTTY.
pub fn header_line(now: &chrono::DateTime<chrono::Local>) -> String {
    format!(
        "=~=~=~=~=~=~=~=~=~=~=~= Reach log {} =~=~=~=~=~=~=~=~=~=~=~=\r\n",
        now.format("%Y.%m.%d %H:%M:%S")
    )
}

/// Collects what a terminal would print.
struct Printable(Vec<u8>);

impl vte::Perform for Printable {
    fn print(&mut self, c: char) {
        let mut b = [0u8; 4];
        self.0.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
    }

    fn execute(&mut self, byte: u8) {
        if matches!(byte, b'\r' | b'\n' | b'\t') {
            self.0.push(byte);
        }
    }
}

/// What goes to a log's writer thread.
enum ToWriter {
    Data(Vec<u8>),
    /// Stop logging: close the file now.
    Close,
}

const ON: u8 = 0;
const PAUSED: u8 = 1;
const STOPPED: u8 = 2;

/// The part of a log the bottom bar can reach while the terminal owns it.
struct Control {
    state: AtomicU8,
    tx: mpsc::Sender<ToWriter>,
    path: PathBuf,
    header: bool,
}

/// A log's state, for the bottom-bar indicator.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogStatus {
    pub path: String,
    pub paused: bool,
}

/// The open logs, by terminal stream (an SSH connection id, or the stream a
/// history tap records).
fn registry() -> &'static Mutex<HashMap<String, Arc<Control>>> {
    static REG: OnceLock<Mutex<HashMap<String, Arc<Control>>>> = OnceLock::new();
    REG.get_or_init(Default::default)
}

fn status_of(c: &Control) -> LogStatus {
    LogStatus { path: c.path.display().to_string(), paused: c.state.load(Ordering::Relaxed) == PAUSED }
}

/// A marker line, so a gap in the log is visible when it is read later.
fn marker(what: &str) -> Vec<u8> {
    format!("\r\n=~=~=~=~=~=~=~=~=~=~=~= Reach log {} {} =~=~=~=~=~=~=~=~=~=~=~=\r\n", what, chrono::Local::now().format("%Y.%m.%d %H:%M:%S")).into_bytes()
}

/// The log of `stream`, if it is writing or paused.
pub fn log_status(stream: &str) -> Option<LogStatus> {
    registry().lock().unwrap_or_else(|e| e.into_inner()).get(stream).map(|c| status_of(c))
}

/// Pauses or resumes the log of `stream`. Output while paused is not
/// written; with the header on, the file marks where it paused and resumed.
pub fn set_log_paused(stream: &str, paused: bool) -> Option<LogStatus> {
    let reg = registry().lock().unwrap_or_else(|e| e.into_inner());
    let c = reg.get(stream)?;
    let (from, to, what) = if paused { (ON, PAUSED, "paused") } else { (PAUSED, ON, "resumed") };
    if c.state.compare_exchange(from, to, Ordering::Relaxed, Ordering::Relaxed).is_ok() && c.header {
        let _ = c.tx.send(ToWriter::Data(marker(what)));
    }
    Some(status_of(c))
}

/// Stops the log of `stream` for good: the file is closed and nothing more
/// is written to it while the tab lives. Returns whether there was one.
pub fn stop_log(stream: &str) -> bool {
    let Some(c) = registry().lock().unwrap_or_else(|e| e.into_inner()).remove(stream) else {
        return false;
    };
    if c.state.swap(STOPPED, Ordering::Relaxed) != STOPPED {
        if c.header {
            let _ = c.tx.send(ToWriter::Data(marker("stopped")));
        }
        let _ = c.tx.send(ToWriter::Close);
    }
    true
}

/// One session's log. Writing happens on its own thread, so a slow disk can
/// never hold up the terminal; dropping this closes the file once everything
/// sent has been written.
pub struct SessionLog {
    mode: LogMode,
    parser: vte::Parser,
    control: Arc<Control>,
    /// The stream it is registered under, for pause and stop.
    stream: Option<String>,
}

impl SessionLog {
    pub fn open(cfg: &SessionLogConfig, host: &str, port: u16) -> io::Result<SessionLog> {
        let now = chrono::Local::now();
        let path = resolve_path(cfg, host, port, &now)?;
        if let Some(dir) = path.parent() {
            create_dirs(dir)?;
        }
        let mut file = open_file(&path, cfg.append)?;
        if cfg.header {
            file.write_all(header_line(&now).as_bytes())?;
            file.flush()?;
        }
        let (tx, rx) = mpsc::channel::<ToWriter>();
        let shown = path.display().to_string();
        std::thread::Builder::new().name("ssh-log".into()).spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut ok = true;
                let mut close = false;
                for msg in std::iter::once(first).chain(std::iter::from_fn(|| rx.try_recv().ok())) {
                    match msg {
                        ToWriter::Data(b) => ok = ok && file.write_all(&b).is_ok(),
                        ToWriter::Close => {
                            close = true;
                            break;
                        }
                    }
                }
                // PuTTY's "flush log file frequently", which is its default.
                if !ok || file.flush().is_err() {
                    tracing::error!("session log: writing {} failed, logging stopped", shown);
                    return;
                }
                if close {
                    return;
                }
            }
        })?;
        let control = Arc::new(Control { state: AtomicU8::new(ON), tx, path, header: cfg.header });
        Ok(SessionLog { mode: cfg.mode, parser: vte::Parser::new(), control, stream: None })
    }

    /// Lets the bottom bar pause and stop this log by `stream`.
    pub fn register(&mut self, stream: &str) {
        registry().lock().unwrap_or_else(|e| e.into_inner()).insert(stream.to_string(), self.control.clone());
        self.stream = Some(stream.to_string());
    }

    pub fn path(&self) -> &Path {
        &self.control.path
    }

    /// Whether it still writes (not stopped from the bottom bar).
    pub fn is_open(&self) -> bool {
        self.control.state.load(Ordering::Relaxed) != STOPPED
    }

    /// Logs output exactly as it is about to be shown.
    pub fn push(&mut self, text: &str) {
        if self.control.state.load(Ordering::Relaxed) != ON {
            return;
        }
        let bytes = match self.mode {
            LogMode::All => text.as_bytes().to_vec(),
            LogMode::Printable => {
                let mut p = Printable(Vec::new());
                self.parser.advance(&mut p, text.as_bytes());
                p.0
            }
        };
        if !bytes.is_empty() {
            let _ = self.control.tx.send(ToWriter::Data(bytes));
        }
    }
}

impl Drop for SessionLog {
    fn drop(&mut self) {
        // Only its own entry: a reconnect may have registered a new log
        // under the same stream.
        if let Some(stream) = &self.stream {
            let mut reg = registry().lock().unwrap_or_else(|e| e.into_inner());
            if reg.get(stream).is_some_and(|c| Arc::ptr_eq(c, &self.control)) {
                reg.remove(stream);
            }
        }
    }
}

#[cfg(test)]
#[path = "session_log_tests.rs"]
mod tests;
