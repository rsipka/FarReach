//! The serial port surface the terminal and the session editor call; see
//! [`crate::serial`]. Desktop only.

use crate::serial::port::{self, SerialConnectParams, SerialPortInfo};
use crate::state::AppState;

/// The serial ports the system knows about.
#[tauri::command]
pub async fn serial_list_ports() -> Result<Vec<SerialPortInfo>, String> {
    tauri::async_runtime::spawn_blocking(port::list_ports)
        .await
        .map_err(|e| e.to_string())?
}

/// Open the port, then stream output as `serial-data-{id}` events until
/// `serial-exit-{id}`. Returns once the port is open, or why it is not.
#[tauri::command]
pub async fn serial_connect(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    params: SerialConnectParams,
) -> Result<(), String> {
    tracing::info!(
        "Serial {}: opening {} at {} baud",
        params.id,
        params.port,
        params.options.baud_rate
    );
    let (name, options) = (params.port.clone(), params.options);
    let opened = tauri::async_runtime::spawn_blocking(move || port::open(&name, &options))
        .await
        .map_err(|e| e.to_string())??;
    state.serial_manager.lock().await.start(app_handle, params, opened)
}

#[tauri::command]
pub async fn serial_send(
    state: tauri::State<'_, AppState>,
    id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    // A ZMODEM transfer holds the line.
    if crate::zmodem::intercept_input(&id, &data) {
        return Ok(());
    }
    crate::history::input(&id, &data);
    state.serial_manager.lock().await.send(&id, data)
}

/// Hold a break on the line (a Cisco ROMMON or Juniper boot loader asks for one).
#[tauri::command]
pub async fn serial_break(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state.serial_manager.lock().await.send_break(&id)
}

#[tauri::command]
pub async fn serial_disconnect(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let threads = state.serial_manager.lock().await.close(&id);
    // Returns once the port is free, so it can be opened again at once.
    tauri::async_runtime::spawn_blocking(move || {
        for t in threads {
            let _ = t.join();
        }
    })
    .await
    .map_err(|e| e.to_string())
}
