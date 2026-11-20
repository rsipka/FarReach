//! The password history of sessions and saved credentials (see
//! `ipc::password_history`): read for the editor, and cleared there or
//! from Settings. Clearing edits each record as stored, so every other
//! field stays as it was, including fields a newer build may have added.

use super::*;
use crate::state::PastPassword;

/// Read a stored record as JSON, let `edit` change it, and write it back
/// only when `edit` says it changed something.
async fn edit_record(
    manager: &crate::vault::VaultManager,
    vault_id: &str,
    row_id: &str,
    edit: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>) -> bool,
) -> Result<(), String> {
    let plaintext = manager.read_secret(vault_id, row_id).await.map_err(|e| e.to_string())?;
    use secrecy::ExposeSecret;
    let mut value: serde_json::Value =
        serde_json::from_slice(plaintext.expose_secret()).map_err(|e| format!("Invalid record: {e}"))?;
    let Some(obj) = value.as_object_mut() else { return Err(format!("Invalid record: {row_id}")) };
    if !edit(obj) {
        return Ok(());
    }
    let json = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    manager
        .update_secret(vault_id, row_id, SecretBox::new(Box::new(json)))
        .await
        .map_err(|e| e.to_string())
}

/// Where a session (`credential` false) or a saved credential is stored.
async fn locate_record(manager: &crate::vault::VaultManager, id: &str, credential: bool) -> Result<SessionLoc, String> {
    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }
    let found = if credential {
        // Credentials live where sessions do: this device's sessions vault
        // (under their own id or the personal-sync copy's), or a user vault.
        let mut found = None;
        if let Some(vault_id) = get_sessions_vault_id_if_exists(manager) {
            if let Some(secret_id) = session_row_id(manager, &vault_id, id).await {
                found = Some(SessionLoc { vault_id, secret_id });
            }
        }
        match found {
            Some(f) => Some(f),
            None => find_in_user_vaults(manager, id)
                .await
                .map(|vault_id| SessionLoc { vault_id, secret_id: id.to_string() }),
        }
    } else {
        locate_session(manager, id).await
    };
    found.ok_or_else(|| format!("Not found: {id}"))
}

async fn read_history(manager: &crate::vault::VaultManager, id: &str, credential: bool) -> Result<Vec<PastPassword>, String> {
    let loc = locate_record(manager, id, credential).await?;
    let mut out = Vec::new();
    edit_record(manager, &loc.vault_id, &loc.secret_id, |obj| {
        out = pwh::read_in(obj);
        false
    })
    .await?;
    Ok(out)
}

async fn clear_history(manager: &crate::vault::VaultManager, id: &str, credential: bool) -> Result<(), String> {
    let loc = locate_record(manager, id, credential).await?;
    edit_record(manager, &loc.vault_id, &loc.secret_id, pwh::clear_in).await
}

/// The passwords a session used before, newest first.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_password_history(state: State<'_, AppState>, session_id: String) -> Result<Vec<PastPassword>, String> {
    let manager = state.vault_manager.lock().await;
    read_history(&manager, &session_id, false).await
}

/// Forget the passwords a session used before.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn session_password_history_clear(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    clear_history(&manager, &session_id, false).await?;
    tracing::info!("Cleared the password history of session {session_id}");
    Ok(())
}

/// The passwords a saved credential held before, newest first.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn saved_credential_password_history(
    state: State<'_, AppState>,
    credential_id: String,
) -> Result<Vec<PastPassword>, String> {
    let manager = state.vault_manager.lock().await;
    read_history(&manager, &credential_id, true).await
}

/// Forget the passwords a saved credential held before.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn saved_credential_password_history_clear(state: State<'_, AppState>, credential_id: String) -> Result<(), String> {
    let manager = state.vault_manager.lock().await;
    clear_history(&manager, &credential_id, true).await?;
    tracing::info!("Cleared the password history of credential {credential_id}");
    Ok(())
}

/// Forget every old password of every session and saved credential in
/// every open vault (a shared vault's too: for all its members). Returns
/// how many records had one.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn password_history_clear_all(state: State<'_, AppState>) -> Result<u32, String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }
    let categories: &[&str] = &["session", "custom:session", "credential"];
    let mut vaults: Vec<String> = Vec::new();
    if let Some(v) = get_sessions_vault_id_if_exists(&manager) {
        vaults.push(v);
    }
    for v in manager.list_vaults().await.unwrap_or_default() {
        if !v.name.starts_with("__") {
            vaults.push(v.id);
        }
    }
    let mut cleared = 0u32;
    for vault_id in vaults {
        let Ok(secrets) = manager.read_secrets_in(&vault_id, categories).await else { continue };
        for (meta, plaintext) in secrets {
            let Ok(plaintext) = plaintext else { continue };
            use secrecy::ExposeSecret;
            let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(plaintext.expose_secret()) else { continue };
            let Some(obj) = value.as_object_mut() else { continue };
            if !pwh::clear_in(obj) {
                continue;
            }
            let json = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
            manager
                .update_secret(&vault_id, &meta.id, SecretBox::new(Box::new(json)))
                .await
                .map_err(|e| e.to_string())?;
            cleared += 1;
        }
    }
    tracing::info!("Cleared the password history of {cleared} sessions and credentials");
    Ok(cleared)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_password(id: &str, password: &str) -> SessionConfig {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "host": "h.example.com", "port": 22, "username": "u",
            "auth_method": { "type": "Password", "password": password }, "folder_id": null, "tags": []
        }))
        .unwrap()
    }

    /// Save `session` as `session_update` does.
    async fn save(m: &crate::vault::VaultManager, session: SessionConfig) {
        let (loc, prepared) = prepare_update(m, session).await.unwrap();
        let json = serde_json::to_vec(&prepared.record).unwrap();
        m.update_secret(&loc.vault_id, &loc.secret_id, SecretBox::new(Box::new(json))).await.unwrap();
    }

    fn passwords(list: &[PastPassword]) -> Vec<&str> {
        list.iter().map(|p| p.password.as_str()).collect()
    }

    #[tokio::test]
    async fn saving_a_new_password_keeps_the_old_one_and_use_restores_it() {
        let dir = std::env::temp_dir().join(format!("reach-pw-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut m = crate::vault::VaultManager::new(dir.clone());
        m.init_identity("pw-history").await.unwrap();
        let private = ensure_sessions_vault(&mut m).await.unwrap();
        let first = serde_json::to_vec(&with_password("s", "one")).unwrap();
        m.create_secret_with_id(&private, "s", "s", SecretCategory::Session, SecretBox::new(Box::new(first))).await.unwrap();

        // A save that changes nothing keeps no history.
        save(&m, with_password("s", "one")).await;
        assert!(read_history(&m, "s", false).await.unwrap().is_empty());

        // What the window sends as history is never stored.
        let mut sent = with_password("s", "two");
        sent.password_history = Some(vec![PastPassword { password: "planted".into(), replaced_at: 1 }]);
        save(&m, sent).await;
        assert_eq!(passwords(&read_history(&m, "s", false).await.unwrap()), ["one"]);

        save(&m, with_password("s", "three")).await;
        assert_eq!(passwords(&read_history(&m, "s", false).await.unwrap()), ["two", "one"]);

        // Putting "one" back: it leaves the list, and "three" goes in front.
        save(&m, with_password("s", "one")).await;
        assert_eq!(passwords(&read_history(&m, "s", false).await.unwrap()), ["three", "two"]);
        assert_eq!(pwh::typed_password(&read_session(&m, &private, "s").await.unwrap().auth_method), Some("one"));

        // The limit setting trims it on the next change; Off keeps none.
        crate::ipc::settings_commands::write_setting_by_key(&mut m, pwh::LIMIT_SETTING, "1").await.unwrap();
        save(&m, with_password("s", "four")).await;
        assert_eq!(passwords(&read_history(&m, "s", false).await.unwrap()), ["one"]);

        clear_history(&m, "s", false).await.unwrap();
        assert!(read_history(&m, "s", false).await.unwrap().is_empty());
        assert_eq!(pwh::typed_password(&read_session(&m, &private, "s").await.unwrap().auth_method), Some("four"), "clearing keeps the password");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
