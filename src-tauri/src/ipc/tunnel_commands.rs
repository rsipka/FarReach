use crate::ipc::{session_commands::stored_session, ssh_commands};
use crate::plugin::{hooks, schema::HookEvent};
use crate::state::{AppState, TunnelConfig, TunnelType};
use crate::tunnel::manager::TunnelManager;

#[tauri::command]
pub async fn tunnel_create(
    state: tauri::State<'_, AppState>,
    tunnel_type: TunnelType,
    local_port: u16,
    remote_host: String,
    remote_port: u16,
    session_id: String,
) -> Result<TunnelConfig, String> {
    if local_port == 0 {
        return Err("Local port must be between 1 and 65535".into());
    }
    let remote_host = remote_host.trim();
    let (remote_host, remote_port) = match tunnel_type {
        TunnelType::Local if remote_host.is_empty() || remote_port == 0 => {
            return Err("A local forward needs a destination host and port".into())
        }
        TunnelType::Local => (remote_host.to_owned(), remote_port),
        TunnelType::Dynamic => (String::new(), 0),
    };
    let session = stored_session(&state, &session_id).await?;
    ssh_commands::require_ssh_session(&session)?;
    Ok(state.tunnel_manager.lock().await.create(
        tunnel_type,
        local_port,
        remote_host,
        remote_port,
        session_id,
    ))
}

#[tauri::command]
pub async fn tunnel_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    tunnel_id: String,
) -> Result<(), String> {
    let (config, starting) = state
        .tunnel_manager
        .lock()
        .await
        .reserve_start(&tunnel_id)
        .map_err(|e| e.to_string())?;
    let ssh = ssh_commands::open_saved_headless(&app, &state, &config.session_id, None).await?;
    let listener = TunnelManager::bind(config.local_port)
        .await
        .map_err(|e| e.to_string())?;
    state
        .tunnel_manager
        .lock()
        .await
        .start(&tunnel_id, listener, ssh)
        .map_err(|e| e.to_string())?;
    drop(starting);
    dispatch(
        &app,
        &state,
        hooks::tunnel_started(&tunnel_id, config.local_port),
    );
    Ok(())
}

#[tauri::command]
pub async fn tunnel_stop(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    tunnel_id: String,
) -> Result<(), String> {
    stop(&app, &state, &tunnel_id, false).await
}

#[tauri::command]
pub async fn tunnel_remove(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    tunnel_id: String,
) -> Result<(), String> {
    stop(&app, &state, &tunnel_id, true).await
}

async fn stop(
    app: &tauri::AppHandle,
    state: &AppState,
    id: &str,
    remove: bool,
) -> Result<(), String> {
    let was_running = state
        .tunnel_manager
        .lock()
        .await
        .stop(id, remove)
        .await
        .map_err(|err| err.to_string())?;
    if was_running {
        dispatch(app, state, hooks::tunnel_stopped(id));
    }
    Ok(())
}

fn dispatch(app: &tauri::AppHandle, state: &AppState, hook: HookEvent) {
    let manager = state.plugin_manager.clone();
    let app = app.clone();
    tokio::spawn(async move {
        manager.lock().await.dispatch_hook(&hook, Some(&app)).await;
    });
}

#[tauri::command]
pub async fn tunnel_list(state: tauri::State<'_, AppState>) -> Result<Vec<TunnelConfig>, String> {
    Ok(state.tunnel_manager.lock().await.list())
}
