//! Editing many sessions at once.
//!
//! The window sends the ids of the selected sessions and a patch naming only
//! the fields the user changed; every other field of each session stays as
//! it is stored. Each session goes through the same steps as a save from the
//! session editor (`prepare_update`): credential links are checked, the
//! user's password manager approvals are kept, and the login a credential
//! fills in is not stored as the session's own. Fields a newer build may
//! have added to a record are written back untouched.
//!
//! Sessions are written one by one: one that fails (not found, a credential
//! from another vault, a shared vault that refuses the write) is reported
//! and the others stay written. The records as they were before are kept in
//! memory, never on disk and never sent to the window, so the last bulk edit
//! can be undone until the app quits, the vault locks or another bulk edit
//! runs.

use super::*;
use crate::state::SessionKind;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// One field of the patch: give it a value on every session, or take it
/// away. A field the user left alone is absent from the patch.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Change<T> {
    Set(T),
    Clear,
}

/// What happens to the notes.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum NotesChange {
    Replace { text: String },
    /// Added as a new line after what is there.
    Append { text: String },
    Clear,
}

/// The fields to change on every selected session.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SessionPatch {
    pub username: Option<String>,
    /// The session's own password. Only sessions that log in with a
    /// password take it; key and agent sessions are left alone.
    pub password: Option<Change<String>>,
    /// `Set(id)` links a saved credential, `Set("own")` uses the session's
    /// own login even where its folder names a credential, `Clear` goes back
    /// to the folder's credential, if any.
    pub credential: Option<Change<String>>,
    pub port: Option<u16>,
    /// Windows logon domain, RDP only.
    pub domain: Option<Change<String>>,
    pub tags_add: Vec<String>,
    pub tags_remove: Vec<String>,
    pub notes: Option<NotesChange>,
    /// A terminal theme id; `Clear`: the default from Settings.
    pub terminal_theme: Option<Change<String>>,
    /// `off`, `commands` or `output`; `Clear`: the default from Settings.
    pub history: Option<Change<String>>,
    /// `block`, `underline` or `bar`; `Clear`: the default from Settings.
    pub cursor_style: Option<Change<String>>,
    /// The cursor blinks or not; `Clear`: the default from Settings.
    pub cursor_blink: Option<Change<bool>>,
}

impl SessionPatch {
    fn check(&self) -> Result<(), String> {
        if let Some(Change::Set(h)) = &self.history {
            if !matches!(h.as_str(), "off" | "commands" | "output") {
                return Err(format!("Unknown history mode: {h}"));
            }
        }
        if let Some(Change::Set(c)) = &self.cursor_style {
            if !matches!(c.as_str(), "block" | "underline" | "bar") {
                return Err(format!("Unknown cursor shape: {c}"));
            }
        }
        if self.port == Some(0) {
            return Err("Port 0 is not a port".into());
        }
        Ok(())
    }
}

/// Fields a kind of session has. The window offers the same rules, as
/// "applies to 8 of 24".
fn has_login(kind: SessionKind) -> bool {
    !matches!(kind, SessionKind::Wsl | SessionKind::Serial)
}

/// The kinds the session editor offers a saved credential for.
fn takes_credential(kind: SessionKind) -> bool {
    matches!(kind, SessionKind::Ssh | SessionKind::Rdp | SessionKind::Telnet)
}

fn has_terminal(kind: SessionKind) -> bool {
    matches!(kind, SessionKind::Ssh | SessionKind::Telnet | SessionKind::Wsl | SessionKind::Serial)
}

/// Apply `patch` to `s`, a session as the editor reads it. `uses_credential`
/// says whether a saved credential would fill in the login of the session
/// as it is then (after the credential part of the patch): its username and
/// password are the credential's, so the patch's are not used. Returns the
/// names of the fields the session does not take.
fn apply_patch(s: &mut SessionConfig, patch: &SessionPatch, uses_credential: impl Fn(&SessionConfig) -> bool) -> Vec<&'static str> {
    let mut skipped = Vec::new();
    let login = has_login(s.kind);

    if let Some(c) = &patch.credential {
        if takes_credential(s.kind) {
            s.credential_id = match c {
                Change::Set(id) => Some(id.clone()),
                Change::Clear => None,
            };
        } else {
            skipped.push("credential");
        }
    }
    let from_credential = takes_credential(s.kind) && uses_credential(s);

    if let Some(u) = &patch.username {
        if login && !from_credential {
            s.username = u.trim().to_string();
        } else {
            skipped.push("username");
        }
    }
    if let Some(p) = &patch.password {
        match &mut s.auth_method {
            AuthMethod::Password { password, password_ref } if login && !from_credential => {
                *password = match p {
                    Change::Set(p) => Some(p.clone()),
                    Change::Clear => None,
                };
                *password_ref = None;
            }
            _ => skipped.push("password"),
        }
    }
    if let Some(port) = patch.port {
        if login {
            s.port = port;
        } else {
            skipped.push("port");
        }
    }
    if let Some(d) = &patch.domain {
        if s.kind == SessionKind::Rdp {
            s.domain = match d {
                Change::Set(d) if !d.trim().is_empty() => Some(d.trim().to_string()),
                _ => None,
            };
        } else {
            skipped.push("domain");
        }
    }

    for tag in patch.tags_add.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        if !s.tags.iter().any(|t| t == tag) {
            s.tags.push(tag.to_string());
        }
    }
    if !patch.tags_remove.is_empty() {
        s.tags.retain(|t| !patch.tags_remove.iter().any(|r| r.trim() == t));
    }
    match &patch.notes {
        None => {}
        Some(NotesChange::Clear) => s.notes = None,
        Some(NotesChange::Replace { text }) => s.notes = Some(text.clone()),
        Some(NotesChange::Append { text }) => {
            s.notes = Some(match s.notes.as_deref().map(str::trim_end).filter(|n| !n.is_empty()) {
                Some(n) => format!("{n}\n{text}"),
                None => text.clone(),
            });
        }
    }

    let terminal = has_terminal(s.kind);
    if let Some(t) = &patch.terminal_theme {
        if terminal {
            s.terminal_theme = match t {
                Change::Set(t) if !t.trim().is_empty() => Some(t.trim().to_string()),
                _ => None,
            };
        } else {
            skipped.push("terminal_theme");
        }
    }
    if let Some(h) = &patch.history {
        if terminal {
            s.history = match h {
                Change::Set(h) => Some(h.clone()),
                Change::Clear => None,
            };
        } else {
            skipped.push("history");
        }
    }
    if let Some(c) = &patch.cursor_style {
        if terminal {
            s.cursor_style = match c {
                Change::Set(c) => Some(c.clone()),
                Change::Clear => None,
            };
        } else {
            skipped.push("cursor_style");
        }
    }
    if let Some(b) = &patch.cursor_blink {
        if terminal {
            s.cursor_blink = match b {
                Change::Set(b) => Some(*b),
                Change::Clear => None,
            };
        } else {
            skipped.push("cursor_blink");
        }
    }
    s.tidy_documentation();
    skipped
}

/// The record to write: `record` with the keys of `raw` this build does not
/// know (a newer Reach may have added fields) put back as they were.
fn with_unknown_fields(raw: &serde_json::Value, record: &SessionConfig) -> Result<serde_json::Value, String> {
    let mut out = serde_json::to_value(record).map_err(|e| e.to_string())?;
    let Some(raw_obj) = raw.as_object() else { return Ok(out) };
    let known = serde_json::from_value::<SessionConfig>(raw.clone())
        .ok()
        .and_then(|s| serde_json::to_value(s).ok())
        .and_then(|v| v.as_object().map(|o| o.keys().cloned().collect::<std::collections::HashSet<_>>()))
        .unwrap_or_default();
    if let Some(out_obj) = out.as_object_mut() {
        for (k, v) in raw_obj {
            if !known.contains(k) && !out_obj.contains_key(k) {
                out_obj.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(out)
}

/// What happened to one session.
#[derive(Debug, Serialize)]
pub struct BulkOutcome {
    pub id: String,
    pub name: String,
    /// `changed`, `unchanged` or `failed`.
    pub status: &'static str,
    /// Patch fields this session does not take (see `apply_patch`).
    pub skipped: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BulkResult {
    pub results: Vec<BulkOutcome>,
    /// Pass to `session_bulk_undo`; absent when nothing changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undo: Option<String>,
}

struct UndoEntry {
    vault_id: String,
    secret_id: String,
    name: String,
    before: SecretBox<Vec<u8>>,
    /// SHA-256 of the record as the bulk edit wrote it: undo restores a
    /// session only if it is still that.
    after: Vec<u8>,
}

struct Undo {
    token: String,
    entries: Vec<UndoEntry>,
}

static LAST_EDIT: Mutex<Option<Undo>> = Mutex::new(None);

fn digest(bytes: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).to_vec()
}

/// Drop the records kept for undo. Called when the vault locks.
pub(crate) fn forget_bulk_undo() {
    if let Ok(mut last) = LAST_EDIT.lock() {
        *last = None;
    }
}

async fn edit_one(
    manager: &crate::vault::VaultManager,
    id: &str,
    patch: &SessionPatch,
) -> Result<(BulkOutcome, Option<UndoEntry>), (String, String)> {
    use secrecy::ExposeSecret;
    let fail = |name: &str, e: String| (name.to_string(), e);
    let loc = locate_session(manager, id).await.ok_or_else(|| fail("", format!("Session not found: {id}")))?;
    let raw_bytes = manager.read_secret(&loc.vault_id, &loc.secret_id).await.map_err(|e| fail("", e.to_string()))?;
    let raw: serde_json::Value =
        serde_json::from_slice(raw_bytes.expose_secret()).map_err(|e| fail("", format!("Invalid session data: {e}")))?;

    // The session as the editor would show it, then the patch on top.
    let (mut session, _) = read_session_filled(manager, &loc).await.map_err(|e| fail("", e))?;
    open_approvals(manager, &mut session).await;
    let name = session.name.clone();
    let before = serde_json::to_value(&session).map_err(|e| fail(&name, e.to_string()))?;

    let logins = creds::logins(manager).await;
    let vault = creds::vault_of(manager, &loc.vault_id);
    let skipped = apply_patch(&mut session, patch, |s| {
        let mut probe = s.clone();
        creds::fill(&mut probe, vault.as_deref(), &logins.folders, &logins.creds);
        probe.credential.is_some_and(|c| !c.missing)
    });
    let after = serde_json::to_value(&session).map_err(|e| fail(&name, e.to_string()))?;
    if after == before {
        return Ok((BulkOutcome { id: id.to_string(), name, status: "unchanged", skipped, error: None }, None));
    }

    let (loc, prepared) = prepare_update(manager, session).await.map_err(|e| fail(&name, e))?;
    let record = with_unknown_fields(&raw, &prepared.record).map_err(|e| fail(&name, e))?;
    let bytes = serde_json::to_vec(&record).map_err(|e| fail(&name, e.to_string()))?;
    let after = digest(&bytes);
    manager
        .update_secret(&loc.vault_id, &loc.secret_id, SecretBox::new(Box::new(bytes)))
        .await
        .map_err(|e| fail(&name, e.to_string()))?;
    let entry = UndoEntry {
        vault_id: loc.vault_id,
        secret_id: loc.secret_id,
        name: name.clone(),
        before: SecretBox::new(Box::new(raw_bytes.expose_secret().clone())),
        after,
    };
    Ok((BulkOutcome { id: id.to_string(), name, status: "changed", skipped, error: None }, Some(entry)))
}

/// Change the same fields on many sessions (see the module notes).
#[tauri::command]
// The patch can carry a password; only the count is logged.
#[tracing::instrument(skip_all, fields(count = session_ids.len()))]
pub async fn session_bulk_update(
    state: State<'_, AppState>,
    session_ids: Vec<String>,
    patch: SessionPatch,
) -> Result<BulkResult, String> {
    patch.check()?;
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }
    let mut results = Vec::with_capacity(session_ids.len());
    let mut entries = Vec::new();
    for id in &session_ids {
        match edit_one(&manager, id, &patch).await {
            Ok((outcome, entry)) => {
                results.push(outcome);
                entries.extend(entry);
            }
            Err((name, error)) => {
                tracing::warn!("Bulk edit of session {id} failed: {error}");
                results.push(BulkOutcome { id: id.clone(), name, status: "failed", skipped: Vec::new(), error: Some(error) });
            }
        }
    }
    let changed = entries.len();
    let undo = (!entries.is_empty()).then(|| uuid::Uuid::new_v4().to_string());
    if let Ok(mut last) = LAST_EDIT.lock() {
        *last = undo.clone().map(|token| Undo { token, entries });
    }
    tracing::info!("Bulk edit changed {changed} of {} sessions", session_ids.len());
    Ok(BulkResult { results, undo })
}

#[derive(Debug, Serialize)]
pub struct UndoResult {
    pub restored: u32,
    /// Sessions changed again since the bulk edit (or gone), left as they are.
    pub kept: Vec<String>,
}

/// Put back the sessions a bulk edit changed, as they were before it,
/// except any changed since. Works once, for the last bulk edit only.
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn session_bulk_undo(state: State<'_, AppState>, token: String) -> Result<UndoResult, String> {
    use secrecy::ExposeSecret;
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Vault is locked".to_string());
    }
    let undo = {
        let mut last = LAST_EDIT.lock().map_err(|_| "Undo is not available".to_string())?;
        match last.take() {
            Some(u) if u.token == token => u,
            other => {
                *last = other;
                return Err("This bulk edit can no longer be undone".to_string());
            }
        }
    };
    let mut restored = 0u32;
    let mut kept = Vec::new();
    for e in undo.entries {
        let now = manager.read_secret(&e.vault_id, &e.secret_id).await;
        let unchanged = now.as_ref().is_ok_and(|b| digest(b.expose_secret()) == e.after);
        if !unchanged {
            kept.push(e.name);
            continue;
        }
        match manager.update_secret(&e.vault_id, &e.secret_id, e.before).await {
            Ok(()) => restored += 1,
            Err(err) => {
                tracing::warn!("Undo of bulk edit failed for {}: {err}", e.secret_id);
                kept.push(e.name);
            }
        }
    }
    tracing::info!("Bulk edit undone on {restored} sessions, {} kept", kept.len());
    Ok(UndoResult { restored, kept })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(extra: serde_json::Value) -> SessionConfig {
        let mut v = serde_json::json!({
            "id": "s1", "name": "web", "host": "web.example", "port": 22, "username": "u",
            "auth_method": { "type": "Password", "password": "old" }, "folder_id": null, "tags": ["prod"],
        });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value(v).unwrap()
    }

    fn patch(v: serde_json::Value) -> SessionPatch {
        serde_json::from_value(v).unwrap()
    }

    const NO_CREDENTIAL: fn(&SessionConfig) -> bool = |_| false;

    #[test]
    fn the_patch_reads_set_and_clear() {
        let p = patch(serde_json::json!({
            "username": "root", "password": { "set": "pw" }, "domain": "clear",
            "notes": { "op": "append", "text": "x" }, "tags_add": ["a"],
        }));
        assert_eq!(p.username.as_deref(), Some("root"));
        assert_eq!(p.password, Some(Change::Set("pw".into())));
        assert_eq!(p.domain, Some(Change::Clear));
        assert_eq!(p.notes, Some(NotesChange::Append { text: "x".into() }));
        assert!(p.credential.is_none() && p.terminal_theme.is_none());
    }

    #[test]
    fn untouched_fields_stay_and_set_fields_change() {
        let mut s = session(serde_json::json!({ "terminal_theme": "app:dark", "notes": "keep" }));
        let skipped = apply_patch(&mut s, &patch(serde_json::json!({ "username": " root ", "port": 2222 })), NO_CREDENTIAL);
        assert!(skipped.is_empty());
        assert_eq!((s.username.as_str(), s.port), ("root", 2222));
        assert_eq!(s.terminal_theme.as_deref(), Some("app:dark"));
        assert_eq!(s.notes.as_deref(), Some("keep"));
        assert_eq!(s.host, "web.example");
    }

    #[test]
    fn a_password_goes_only_to_password_sessions() {
        let mut pw = session(serde_json::json!({
            "auth_method": { "type": "Password", "password_ref": { "provider": "bitwarden", "entry_id": "x" } },
        }));
        let p = patch(serde_json::json!({ "password": { "set": "new" } }));
        assert!(apply_patch(&mut pw, &p, NO_CREDENTIAL).is_empty());
        match &pw.auth_method {
            AuthMethod::Password { password, password_ref } => {
                assert_eq!(password.as_deref(), Some("new"));
                assert!(password_ref.is_none(), "a typed password replaces the manager entry");
            }
            other => panic!("{other:?}"),
        }
        let mut agent = session(serde_json::json!({ "auth_method": { "type": "Agent" } }));
        assert_eq!(apply_patch(&mut agent, &p, NO_CREDENTIAL), vec!["password"]);
        assert!(matches!(agent.auth_method, AuthMethod::Agent));
    }

    #[test]
    fn a_login_from_a_credential_is_not_overwritten() {
        let mut s = session(serde_json::json!({ "credential_id": "c1" }));
        let p = patch(serde_json::json!({ "username": "root", "password": { "set": "x" } }));
        assert_eq!(apply_patch(&mut s, &p, |s| s.credential_id.as_deref() == Some("c1")), vec!["username", "password"]);
        assert_eq!(s.username, "u");
        // Switching to the session's own login in the same patch lets them through.
        let mut s = session(serde_json::json!({ "credential_id": "c1" }));
        let p = patch(serde_json::json!({ "credential": { "set": "own" }, "username": "root" }));
        assert!(apply_patch(&mut s, &p, |s| s.credential_id.as_deref() == Some("c1")).is_empty());
        assert_eq!((s.credential_id.as_deref(), s.username.as_str()), (Some("own"), "root"));
    }

    #[test]
    fn kind_specific_fields_skip_other_kinds() {
        let p = patch(serde_json::json!({ "domain": { "set": "CORP" }, "terminal_theme": { "set": "scheme:Homebrew" }, "username": "x" }));
        let mut rdp = session(serde_json::json!({ "kind": "rdp" }));
        assert_eq!(apply_patch(&mut rdp, &p, NO_CREDENTIAL), vec!["terminal_theme"]);
        assert_eq!(rdp.domain.as_deref(), Some("CORP"));
        let mut wsl = session(serde_json::json!({ "kind": "wsl" }));
        assert_eq!(apply_patch(&mut wsl, &p, NO_CREDENTIAL), vec!["username", "domain"]);
        assert_eq!(wsl.terminal_theme.as_deref(), Some("scheme:Homebrew"));
        assert_eq!(wsl.username, "u");
        let mut vnc = session(serde_json::json!({ "kind": "vnc" }));
        let c = patch(serde_json::json!({ "credential": { "set": "c1" }, "username": "x" }));
        assert_eq!(apply_patch(&mut vnc, &c, NO_CREDENTIAL), vec!["credential"]);
        assert_eq!((vnc.credential_id, vnc.username.as_str()), (None, "x"));
    }

    #[test]
    fn a_cursor_is_set_and_cleared_on_terminals_only() {
        let p = patch(serde_json::json!({ "cursor_style": { "set": "block" }, "cursor_blink": { "set": false } }));
        assert!(p.check().is_ok());
        let mut s = session(serde_json::json!({}));
        assert!(apply_patch(&mut s, &p, NO_CREDENTIAL).is_empty());
        assert_eq!((s.cursor_style.as_deref(), s.cursor_blink), (Some("block"), Some(false)));
        let clear = patch(serde_json::json!({ "cursor_style": "clear", "cursor_blink": "clear" }));
        apply_patch(&mut s, &clear, NO_CREDENTIAL);
        assert!(s.cursor_style.is_none() && s.cursor_blink.is_none());
        let mut rdp = session(serde_json::json!({ "kind": "rdp" }));
        assert_eq!(apply_patch(&mut rdp, &p, NO_CREDENTIAL), vec!["cursor_style", "cursor_blink"]);
        assert!(patch(serde_json::json!({ "cursor_style": { "set": "beam" } })).check().is_err());
    }

    #[test]
    fn tags_are_added_and_removed_not_replaced() {
        let mut s = session(serde_json::json!({ "tags": ["prod", "db", "eu"] }));
        apply_patch(&mut s, &patch(serde_json::json!({ "tags_add": ["eu", " web "], "tags_remove": ["db"] })), NO_CREDENTIAL);
        assert_eq!(s.tags, vec!["prod", "eu", "web"]);
    }

    #[test]
    fn notes_replace_append_and_clear() {
        let mut s = session(serde_json::json!({ "notes": "Rack 4\n" }));
        apply_patch(&mut s, &patch(serde_json::json!({ "notes": { "op": "append", "text": "Owner: ops" } })), NO_CREDENTIAL);
        assert_eq!(s.notes.as_deref(), Some("Rack 4\nOwner: ops"));
        let mut empty = session(serde_json::json!({}));
        apply_patch(&mut empty, &patch(serde_json::json!({ "notes": { "op": "append", "text": "first" } })), NO_CREDENTIAL);
        assert_eq!(empty.notes.as_deref(), Some("first"));
        apply_patch(&mut s, &patch(serde_json::json!({ "notes": { "op": "clear" } })), NO_CREDENTIAL);
        assert!(s.notes.is_none());
    }

    #[test]
    fn clear_goes_back_to_the_defaults() {
        let mut s = session(serde_json::json!({ "terminal_theme": "custom:1", "history": "output", "credential_id": "c1" }));
        let p = patch(serde_json::json!({ "terminal_theme": "clear", "history": "clear", "credential": "clear" }));
        apply_patch(&mut s, &p, NO_CREDENTIAL);
        assert!(s.terminal_theme.is_none() && s.history.is_none() && s.credential_id.is_none());
    }

    #[test]
    fn a_bad_history_mode_or_port_is_refused() {
        assert!(patch(serde_json::json!({ "history": { "set": "everything" } })).check().is_err());
        assert!(patch(serde_json::json!({ "port": 0 })).check().is_err());
        assert!(patch(serde_json::json!({ "history": { "set": "commands" }, "port": 22 })).check().is_ok());
    }

    #[test]
    fn fields_from_a_newer_build_are_written_back() {
        let raw = serde_json::json!({
            "id": "s1", "name": "web", "host": "web.example", "port": 22, "username": "u",
            "auth_method": { "type": "Agent" }, "folder_id": null, "tags": [],
            "notes": "old", "future_field": { "x": 1 },
        });
        let mut s: SessionConfig = serde_json::from_value(raw.clone()).unwrap();
        s.notes = None;
        s.username = "root".into();
        let out = with_unknown_fields(&raw, &s).unwrap();
        assert_eq!(out["future_field"], serde_json::json!({ "x": 1 }));
        assert_eq!(out["username"], "root");
        // A known field the patch cleared does not come back from the raw record.
        assert!(out.get("notes").is_none());
    }
}
