//! The terminal's answers to a ZMODEM transfer the remote started; see
//! [`crate::zmodem`]. Each takes the session's connection id.

use std::path::PathBuf;

use crate::zmodem::{decide, Decision};

/// The remote ran `rz`: send these files.
#[tauri::command]
pub fn zmodem_upload(id: String, paths: Vec<String>) -> Result<(), String> {
    decide(&id, Decision::Upload(paths.into_iter().map(PathBuf::from).collect()))
}

/// The remote ran `sz`: save into this folder.
#[tauri::command]
pub fn zmodem_download(id: String, folder: String) -> Result<(), String> {
    decide(&id, Decision::Download(PathBuf::from(folder)))
}

/// Decline the transfer, or stop the one running.
#[tauri::command]
pub fn zmodem_cancel(id: String) -> Result<(), String> {
    decide(&id, Decision::Cancel)
}
