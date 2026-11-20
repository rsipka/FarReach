//! Host keys saved on a session, as Remote Desktop Manager keeps them: the
//! server's fingerprint is part of the session record, so it is encrypted,
//! synced and shared with the rest of the session. When a new or changed key
//! comes up, the person picks Accept once (nothing is kept), Accept and save
//! (kept on the session) or Don't connect.
//!
//! Every hop of a connection is kept on the session being connected, under
//! its own `host:port`, so a jump host's key travels with the session that
//! goes through it. A connection without a saved session (Quick Connect,
//! RDP) keeps using this device's own list, `ssh/known_hosts.json`.
//!
//! The device list is still read: a key found there and not yet on the
//! session is copied onto it the first time the session connects, which is
//! how keys accepted by older builds move over.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

/// One server key a session trusts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedHostKey {
    /// The server as it was reached, `host:port` (a jump host's own).
    pub host: String,
    /// `ssh-ed25519`, `rsa-sha2-512`...; empty when typed in by hand.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key_type: String,
    /// `SHA256:` and the key's SHA-256 in base64, without padding.
    pub fingerprint: String,
    /// When it was saved, in seconds since 1970.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_at: Option<i64>,
}

/// What the person answered to a host-key question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Reject,
    /// Connect this time, remember nothing.
    Once,
    /// Connect, and remember the key.
    Save,
}

impl Answer {
    /// From the dialog's two values; a reply without `save` is an older
    /// window's plain Accept, which always saved.
    pub fn from_reply(accept: bool, save: Option<bool>) -> Self {
        match (accept, save.unwrap_or(true)) {
            (false, _) => Answer::Reject,
            (true, false) => Answer::Once,
            (true, true) => Answer::Save,
        }
    }

    pub fn accepted(self) -> bool {
        self != Answer::Reject
    }
}

/// `SHA256:` and base64 for display and storage, from the bare base64 the
/// SSH code works with (or a value that already has the prefix).
pub fn with_prefix(fingerprint: &str) -> String {
    let bare = fingerprint.strip_prefix("SHA256:").unwrap_or(fingerprint);
    format!("SHA256:{bare}")
}

/// The bare base64 of a stored or typed fingerprint.
pub fn bare(fingerprint: &str) -> &str {
    fingerprint.trim().strip_prefix("SHA256:").unwrap_or(fingerprint.trim())
}

/// A fingerprint typed in by hand, as `ssh-keygen -lf` prints it
/// (`SHA256:` and 43 characters of base64, a trailing `=` allowed), in the
/// stored form; `None` when it is not one.
pub fn parse_typed(fingerprint: &str) -> Option<String> {
    let s = fingerprint.trim();
    let b = s.strip_prefix("SHA256:").unwrap_or(s).trim_end_matches('=');
    let ok = b.len() == 43 && b.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'+' || c == b'/');
    ok.then(|| with_prefix(b))
}

/// The saved session a connection is for, with the keys it trusts as they
/// were read when the connection started. Shared by every hop.
#[derive(Debug)]
pub struct SessionTrust {
    pub session_id: String,
    saved: Mutex<Vec<SavedHostKey>>,
}

impl SessionTrust {
    pub fn new(session_id: impl Into<String>, saved: Vec<SavedHostKey>) -> Arc<Self> {
        Arc::new(Self { session_id: session_id.into(), saved: Mutex::new(saved) })
    }

    /// The bare fingerprint saved for `host_id`, if any.
    pub fn lookup(&self, host_id: &str) -> Option<String> {
        let saved = self.saved.lock().unwrap_or_else(|e| e.into_inner());
        saved.iter().find(|k| k.host == host_id).map(|k| bare(&k.fingerprint).to_string())
    }

    /// Keep `fingerprint` (bare) for `host_id` on the session: at once for
    /// the rest of this connection, and in its vault in the background, so
    /// the handshake does not wait on the store. The window is told when
    /// it is written, to show the session as it now is.
    pub fn save(&self, app: Option<&tauri::AppHandle>, host_id: &str, key_type: &str, fingerprint: &str) {
        let entry = SavedHostKey {
            host: host_id.to_string(),
            key_type: key_type.to_string(),
            fingerprint: with_prefix(fingerprint),
            saved_at: Some(now()),
        };
        {
            let mut saved = self.saved.lock().unwrap_or_else(|e| e.into_inner());
            put(&mut saved, entry.clone());
        }
        let Some(app) = app.cloned() else { return };
        let session_id = self.session_id.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<crate::state::AppState>();
            match crate::ipc::session_commands::store_host_key(&state, &session_id, entry).await {
                Ok(()) => {
                    tracing::info!("Saved a host key on session {session_id}");
                    let _ = app.emit("session-host-keys-changed", &session_id);
                }
                Err(e) => tracing::warn!("Could not save the host key on session {session_id}: {e}"),
            }
        });
    }
}

/// Put `entry` in the list, in place of a key kept for the same server.
pub fn put(list: &mut Vec<SavedHostKey>, entry: SavedHostKey) {
    match list.iter_mut().find(|k| k.host == entry.host) {
        Some(k) => *k = entry,
        None => list.push(entry),
    }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_from_the_dialog() {
        assert_eq!(Answer::from_reply(false, Some(true)), Answer::Reject);
        assert_eq!(Answer::from_reply(true, Some(false)), Answer::Once);
        assert_eq!(Answer::from_reply(true, Some(true)), Answer::Save);
        // An older window only knew Accept, which saved.
        assert_eq!(Answer::from_reply(true, None), Answer::Save);
    }

    #[test]
    fn typed_fingerprints() {
        let b = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789+/AbCdE";
        assert_eq!(b.len(), 43);
        assert_eq!(parse_typed(&format!("SHA256:{b}")).as_deref(), Some(format!("SHA256:{b}").as_str()));
        assert_eq!(parse_typed(&format!("  {b}= ")).as_deref(), Some(format!("SHA256:{b}").as_str()));
        assert_eq!(parse_typed("SHA256:short"), None);
        assert_eq!(parse_typed(&format!("MD5:{b}")), None);
        assert_eq!(parse_typed(&format!("SHA256:{}", b.replace('A', "-"))), None);
    }

    #[test]
    fn one_key_per_server() {
        let t = SessionTrust::new("s1", vec![]);
        t.save(None, "example.com:22", "ssh-ed25519", "aaa");
        t.save(None, "jump.example.com:22", "ssh-ed25519", "bbb");
        t.save(None, "example.com:22", "rsa-sha2-512", "SHA256:ccc");
        assert_eq!(t.lookup("example.com:22").as_deref(), Some("ccc"));
        assert_eq!(t.lookup("jump.example.com:22").as_deref(), Some("bbb"));
        assert_eq!(t.lookup("example.com:2222"), None);
        assert_eq!(t.saved.lock().unwrap().len(), 2);
    }
}
