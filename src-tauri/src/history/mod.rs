//! Session history: the commands run and the output printed in terminal
//! tabs, kept in an encrypted local file and searched with FTS5.
//!
//! The terminal registers a tap for its stream (a PTY id, a Telnet id or an
//! SSH connection id) before any data flows, with the level chosen for the
//! session. The read loops and the input commands then call [`output`] and
//! [`input`]; a tap turns them into commands and output blocks
//! ([`capture`]), with secrets removed ([`scrub`]), and a writer task stores
//! them ([`store`]).
//!
//! The file opens with a key kept in the settings vault, so history is
//! readable only while the vault is unlocked. What a terminal records while
//! it is locked waits in memory (up to [`BUFFER_BYTES`]) and is written when
//! it opens again; quitting first drops it.
//!
//! The same tap also writes the PuTTY-style file log for terminals other
//! than SSH (which logs in its own read loop), so every terminal type can
//! have one.

pub mod capture;
pub mod scrub;
pub mod store;

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::sync::mpsc;

use capture::{Capture, Event};
use scrub::Scrubber;
use store::{RunMeta, Retention, Store};

/// The settings-vault entry that holds the file's key, as hex. Never
/// returned by the generic settings commands.
pub const KEY_SETTING: &str = "history.key";
/// What waits in memory while the vault is locked.
pub const BUFFER_BYTES: usize = 16 * 1024 * 1024;
const FILE_NAME: &str = "history.db";
const TICK: Duration = Duration::from_secs(5);
const PRUNE_EVERY_MS: i64 = 6 * 3600 * 1000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Commands,
    Output,
}

/// What the terminal asks for when it opens a stream.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TapConfig {
    pub meta: RunMeta,
    /// None: no history, only (maybe) the file log.
    #[serde(default)]
    pub level: Option<Level>,
    /// Secrets this session uses, to be masked wherever they show up.
    #[serde(default)]
    pub secrets: Vec<String>,
    /// The user's own patterns to mask.
    #[serde(default)]
    pub patterns: Vec<String>,
    /// The file log, for terminals that do not log on their own.
    #[serde(default)]
    pub log: Option<crate::ssh::session_log::SessionLogConfig>,
}

struct Tap {
    run: u64,
    level: Option<Level>,
    capture: Capture,
    log: Option<crate::ssh::session_log::SessionLog>,
    paused: bool,
}

enum Msg {
    Start { run: u64, meta: RunMeta, at: i64 },
    Event { run: u64, event: Event },
    End { run: u64, at: i64 },
    /// The vault locked: close the file.
    Close,
    /// The vault may have unlocked: try to open now.
    Open,
    Configure(Retention),
}

impl Msg {
    fn size(&self) -> usize {
        match self {
            Msg::Event { event: Event::Command { text, .. } | Event::Output { text, .. }, .. } => text.len() + 64,
            _ => 64,
        }
    }
}

/// Whether a tab records, for the indicator.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TapStatus {
    pub recording: Option<Level>,
    pub paused: bool,
    pub log_file: Option<String>,
}

#[derive(Default)]
struct Hub {
    taps: Mutex<HashMap<String, Tap>>,
    tx: OnceLock<mpsc::UnboundedSender<Msg>>,
    next_run: std::sync::atomic::AtomicU64,
}

fn hub() -> &'static Hub {
    static HUB: OnceLock<Hub> = OnceLock::new();
    HUB.get_or_init(Hub::default)
}

fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn send(msg: Msg) {
    if let Some(tx) = hub().tx.get() {
        let _ = tx.send(msg);
    }
}

fn send_events(run: u64, level: Option<Level>, events: Vec<Event>) {
    let Some(level) = level else { return };
    for event in events {
        if level == Level::Commands && matches!(event, Event::Output { .. }) {
            continue;
        }
        send(Msg::Event { run, event });
    }
}

/// Starts recording `stream`. A tap already there stays as it is: a
/// terminal mounted again (a tab moved into a split) asks again.
pub fn start(stream: &str, cfg: TapConfig) -> Result<TapStatus, String> {
    if let Some(st) = status(stream) {
        return Ok(st);
    }
    let log = match &cfg.log {
        Some(l) => {
            let host = if cfg.meta.host.is_empty() { cfg.meta.protocol.as_str() } else { cfg.meta.host.as_str() };
            // `&S` in the name: the tab's session, unless the caller named one.
            let mut l = l.clone();
            if l.session.trim().is_empty() {
                l.session = cfg.meta.name.clone();
            }
            Some(crate::ssh::session_log::SessionLog::open(&l, host, 0).map_err(|e| format!("Session log: {e}"))?)
        }
        None => None,
    };
    if cfg.level.is_none() && log.is_none() {
        return Ok(TapStatus { recording: None, paused: false, log_file: None });
    }
    let (scrubber, _bad) = Scrubber::new(cfg.secrets, &cfg.patterns);
    let run = hub().next_run.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    if cfg.level.is_some() {
        send(Msg::Start { run, meta: cfg.meta, at: now() });
    }
    let status = TapStatus { recording: cfg.level, paused: false, log_file: log.as_ref().map(|l| l.path().display().to_string()) };
    let tap = Tap { run, level: cfg.level, capture: Capture::new(scrubber), log, paused: false };
    hub().taps.lock().unwrap_or_else(|e| e.into_inner()).insert(stream.to_string(), tap);
    Ok(status)
}

/// Stops recording `stream`; what is left is stored.
pub fn stop(stream: &str) {
    let tap = hub().taps.lock().unwrap_or_else(|e| e.into_inner()).remove(stream);
    if let Some(mut tap) = tap {
        let t = now();
        if !tap.paused {
            let events = tap.capture.finish(t);
            send_events(tap.run, tap.level, events);
        }
        if tap.level.is_some() {
            send(Msg::End { run: tap.run, at: t });
        }
    }
}

/// Pauses or resumes recording (the file log goes on: it was asked for
/// separately and has its own setting).
pub fn pause(stream: &str, paused: bool) -> Option<TapStatus> {
    let mut taps = hub().taps.lock().unwrap_or_else(|e| e.into_inner());
    let tap = taps.get_mut(stream)?;
    if paused && !tap.paused {
        let events = tap.capture.finish(now());
        send_events(tap.run, tap.level, events);
    }
    tap.paused = paused;
    Some(status_of(tap))
}

fn status_of(tap: &Tap) -> TapStatus {
    TapStatus { recording: tap.level, paused: tap.paused, log_file: tap.log.as_ref().map(|l| l.path().display().to_string()) }
}

pub fn status(stream: &str) -> Option<TapStatus> {
    hub().taps.lock().unwrap_or_else(|e| e.into_inner()).get(stream).map(status_of)
}

/// Output of `stream`, as it is about to be shown.
pub fn output(stream: &str, text: &str) {
    let mut taps = hub().taps.lock().unwrap_or_else(|e| e.into_inner());
    let Some(tap) = taps.get_mut(stream) else { return };
    if let Some(log) = tap.log.as_mut() {
        log.push(text);
    }
    if tap.paused || tap.level.is_none() {
        return;
    }
    let events = tap.capture.output(now(), text);
    send_events(tap.run, tap.level, events);
}

/// Keys the user sent to `stream`. Never stored as such: see [`capture`].
pub fn input(stream: &str, data: &[u8]) {
    let mut taps = hub().taps.lock().unwrap_or_else(|e| e.into_inner());
    let Some(tap) = taps.get_mut(stream) else { return };
    if tap.paused || tap.level.is_none() {
        return;
    }
    let events = tap.capture.input(now(), data);
    send_events(tap.run, tap.level, events);
}

/// The vault locked: the file closes until it unlocks.
pub fn vault_locked() {
    send(Msg::Close);
}

/// The vault unlocked: buffered history is written.
pub fn vault_unlocked() {
    send(Msg::Open);
}

pub fn configure(keep: Retention) {
    send(Msg::Configure(keep));
}

/// The file's key, made on first use.
async fn key(app: &tauri::AppHandle) -> Result<[u8; 32], String> {
    let state = app.try_state::<crate::state::AppState>().ok_or("history: no app state")?;
    let mut vm = state.vault_manager.lock().await;
    if vm.is_locked() {
        return Err("history: the vault is locked".into());
    }
    if let Some(hex) = crate::ipc::settings_commands::read_setting_by_key(&vm, KEY_SETTING).await {
        let bytes = decode_hex(&hex).ok_or("history: the key in the vault is damaged")?;
        return bytes.try_into().map_err(|_| "history: the key in the vault is damaged".to_string());
    }
    let mut key = [0u8; 32];
    rand::fill(&mut key[..]);
    crate::ipc::settings_commands::write_setting_by_key(&mut vm, KEY_SETTING, &encode_hex(&key)).await?;
    Ok(key)
}

fn encode_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// The open file, shared by the writer and the search commands.
static STORE: tokio::sync::Mutex<Option<Store>> = tokio::sync::Mutex::const_new(None);

/// Opens the file if the vault allows it.
async fn ensure_open(app: &tauri::AppHandle) -> Result<(), String> {
    let mut guard = STORE.lock().await;
    if guard.is_some() {
        return Ok(());
    }
    let key = key(app).await?;
    let path = crate::app_data_dir().join(FILE_NAME);
    *guard = Some(Store::open(&path, key).await?);
    Ok(())
}

/// Runs `f` on the open file, opening it first if the vault is unlocked.
pub async fn with_store<T>(app: &tauri::AppHandle, f: impl AsyncFnOnce(&Store) -> Result<T, String>) -> Result<T, String> {
    ensure_open(app).await?;
    let guard = STORE.lock().await;
    let store = guard.as_ref().ok_or("history: not open")?;
    f(store).await
}

/// Starts the writer. Called once at startup.
pub fn spawn_writer(app: tauri::AppHandle) {
    let (tx, rx) = mpsc::unbounded_channel();
    if hub().tx.set(tx).is_err() {
        return;
    }
    tauri::async_runtime::spawn(writer(app, rx));
    tauri::async_runtime::spawn(async {
        let mut tick = tokio::time::interval(TICK);
        loop {
            tick.tick().await;
            let t = now();
            let mut taps = hub().taps.lock().unwrap_or_else(|e| e.into_inner());
            for tap in taps.values_mut() {
                if !tap.paused {
                    let events = tap.capture.tick(t);
                    send_events(tap.run, tap.level, events);
                }
            }
        }
    });
}

async fn writer(app: tauri::AppHandle, mut rx: mpsc::UnboundedReceiver<Msg>) {
    let mut waiting: VecDeque<Msg> = VecDeque::new();
    let mut waiting_bytes = 0usize;
    let mut runs: HashMap<u64, i64> = HashMap::new();
    let mut metas: HashMap<u64, (RunMeta, i64)> = HashMap::new();
    let mut keep = Retention::default();
    let mut last_prune = 0i64;
    let mut last_try = 0i64;
    let device = gethostname::gethostname().to_string_lossy().into_owned();
    while let Some(msg) = rx.recv().await {
        match msg {
            Msg::Close => {
                *STORE.lock().await = None;
                runs.clear();
                continue;
            }
            Msg::Configure(k) => {
                keep = k;
                last_prune = 0;
                continue;
            }
            Msg::Open => last_try = 0,
            other => {
                waiting_bytes += other.size();
                waiting.push_back(other);
                while waiting_bytes > BUFFER_BYTES {
                    match waiting.pop_front() {
                        Some(m) => waiting_bytes -= m.size(),
                        None => break,
                    }
                }
            }
        }
        // Not open: try at most every 10 s (each try takes the vault lock).
        let t = now();
        if STORE.lock().await.is_none() {
            if t - last_try < 10_000 {
                continue;
            }
            last_try = t;
            if let Err(e) = ensure_open(&app).await {
                tracing::debug!("{e}");
                continue;
            }
            runs.clear();
        }
        let guard = STORE.lock().await;
        let Some(store) = guard.as_ref() else { continue };
        while let Some(msg) = waiting.pop_front() {
            waiting_bytes -= msg.size();
            let r = match msg {
                Msg::Start { run, meta, at } => {
                    metas.insert(run, (meta, at));
                    Ok(())
                }
                Msg::Event { run, event } => {
                    let id = match runs.get(&run) {
                        Some(id) => Ok(*id),
                        None => match metas.get(&run) {
                            Some((meta, at)) => store.new_run(meta, &device, *at).await.inspect(|id| {
                                runs.insert(run, *id);
                            }),
                            None => Err("history: event for an unknown run".to_string()),
                        },
                    };
                    match id {
                        Ok(id) => store.add(id, &event).await,
                        Err(e) => Err(e),
                    }
                }
                Msg::End { run, at } => {
                    metas.remove(&run);
                    match runs.remove(&run) {
                        Some(id) => store.end_run(id, at).await,
                        None => Ok(()),
                    }
                }
                _ => Ok(()),
            };
            if let Err(e) = r {
                tracing::warn!("{e}");
            }
        }
        if t - last_prune > PRUNE_EVERY_MS {
            last_prune = t;
            if let Err(e) = store.prune(t, keep).await {
                tracing::warn!("{e}");
            }
        }
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
