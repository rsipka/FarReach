//! Session history: what the terminal and the search panel call; see
//! [`crate::history`].

use crate::history::store::{Forget, Hit, Query, Retention, Stats};
use crate::history::{TapConfig, TapStatus};

/// Starts recording a terminal stream (a PTY id, a Telnet id, an SSH
/// connection id). Called before the stream's data flows.
#[tauri::command]
pub fn history_start(stream: String, config: TapConfig) -> Result<TapStatus, String> {
    crate::history::start(&stream, config)
}

#[tauri::command]
pub fn history_stop(stream: String) {
    crate::history::stop(&stream);
}

#[tauri::command]
pub fn history_pause(stream: String, paused: bool) -> Option<TapStatus> {
    crate::history::pause(&stream, paused)
}

#[tauri::command]
pub fn history_status(stream: String) -> Option<TapStatus> {
    crate::history::status(&stream)
}

#[tauri::command]
pub async fn history_search(app: tauri::AppHandle, query: Query) -> Result<Vec<Hit>, String> {
    crate::history::with_store(&app, async |s| s.search(&query).await).await
}

#[tauri::command]
pub async fn history_transcript(app: tauri::AppHandle, run: i64) -> Result<Vec<Hit>, String> {
    crate::history::with_store(&app, async |s| s.transcript(run).await).await
}

#[tauri::command]
pub async fn history_forget(app: tauri::AppHandle, what: Forget) -> Result<(), String> {
    crate::history::with_store(&app, async |s| s.forget(&what).await).await
}

#[tauri::command]
pub fn history_configure(retention: Retention) {
    crate::history::configure(retention);
}

#[tauri::command]
pub async fn history_stats(app: tauri::AppHandle) -> Result<Stats, String> {
    crate::history::with_store(&app, async |s| s.stats().await).await
}
