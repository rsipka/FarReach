//! Password history: the passwords a session or a saved credential logged
//! in with before it was changed, so an older one can be looked up or put
//! back (a server restored from a backup, a change that did not reach every
//! machine).
//!
//! The history is a field of the record itself (`password_history`), so it
//! is encrypted with it, synced with it (newest record wins, as for every
//! other field) and goes where the record goes, including a shared vault.
//! It only grows when a save replaces a typed password: what the window
//! sends as history is never stored, and the window reads it only through
//! the commands that show it. Password manager entries are references, not
//! passwords, and have no history.
//!
//! How many are kept is the `password_history_limit` setting (vault-backed,
//! so it follows the user): `DEFAULT_LIMIT` unless set, `0` keeps none.

use crate::state::{AuthMethod, PastPassword};

/// The setting that says how many old passwords each record keeps.
pub(crate) const LIMIT_SETTING: &str = "password_history_limit";
/// Kept when the setting was never changed.
pub(crate) const DEFAULT_LIMIT: usize = 10;
/// The most the setting can ask for.
pub(crate) const MAX_LIMIT: usize = 100;

/// The typed password of a login, if it has one that is not empty.
pub(crate) fn typed_password(auth: &AuthMethod) -> Option<&str> {
    match auth {
        AuthMethod::Password { password: Some(p), .. } if !p.is_empty() => Some(p),
        _ => None,
    }
}

/// The history a record gets when its login goes from `before` to `after`:
/// the stored history, with the password replaced in front when the typed
/// one changed. A password that is current again (restored from the
/// history) leaves it, each password is kept once, and only the newest
/// `limit` stay. `None` when nothing is left.
pub(crate) fn carry(
    stored: Option<&[PastPassword]>,
    before: Option<&str>,
    after: Option<&str>,
    limit: usize,
    now: i64,
) -> Option<Vec<PastPassword>> {
    let mut list: Vec<PastPassword> = Vec::new();
    if let Some(old) = before.filter(|old| Some(*old) != after) {
        list.push(PastPassword { password: old.to_string(), replaced_at: now });
    }
    list.extend(stored.unwrap_or_default().iter().cloned());
    let mut seen = std::collections::HashSet::new();
    list.retain(|p| Some(p.password.as_str()) != after && seen.insert(p.password.clone()));
    list.truncate(limit);
    (!list.is_empty()).then_some(list)
}

/// How many old passwords to keep, as the user set it.
pub(crate) async fn limit(manager: &crate::vault::VaultManager) -> usize {
    let value = crate::ipc::settings_commands::read_setting_by_key(manager, LIMIT_SETTING).await;
    parse_limit(value.as_deref())
}

fn parse_limit(value: Option<&str>) -> usize {
    match value.map(str::trim).and_then(|v| v.parse::<usize>().ok()) {
        Some(n) => n.min(MAX_LIMIT),
        None => DEFAULT_LIMIT,
    }
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Remove the history from a JSON record as stored; true when it had one.
pub(crate) fn clear_in(obj: &mut serde_json::Map<String, serde_json::Value>) -> bool {
    obj.remove("password_history").is_some()
}

/// The history of a JSON record as stored.
pub(crate) fn read_in(obj: &serde_json::Map<String, serde_json::Value>) -> Vec<PastPassword> {
    obj.get("password_history").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn past(p: &str, at: i64) -> PastPassword {
        PastPassword { password: p.into(), replaced_at: at }
    }

    #[test]
    fn a_changed_password_goes_in_front() {
        let stored = [past("one", 1)];
        let got = carry(Some(&stored), Some("two"), Some("three"), 10, 5).unwrap();
        assert_eq!(got, [past("two", 5), past("one", 1)]);
    }

    #[test]
    fn an_unchanged_password_keeps_the_history_as_it_is() {
        let stored = [past("one", 1)];
        assert_eq!(carry(Some(&stored), Some("two"), Some("two"), 10, 5).unwrap(), stored);
        assert_eq!(carry(None, Some("two"), Some("two"), 10, 5), None);
    }

    #[test]
    fn a_first_password_or_one_cleared_is_handled() {
        // Nothing typed before: nothing to keep.
        assert_eq!(carry(None, None, Some("new"), 10, 5), None);
        // Cleared (or moved to a key, an agent, a password manager entry):
        // the old one is kept, so it can be put back.
        assert_eq!(carry(None, Some("old"), None, 10, 5).unwrap(), [past("old", 5)]);
    }

    #[test]
    fn a_restored_password_leaves_the_history() {
        let stored = [past("two", 2), past("one", 1)];
        let got = carry(Some(&stored), Some("three"), Some("one"), 10, 5).unwrap();
        assert_eq!(got, [past("three", 5), past("two", 2)]);
    }

    #[test]
    fn each_password_is_kept_once_newest_first() {
        let stored = [past("one", 2), past("two", 1)];
        let got = carry(Some(&stored), Some("two"), Some("three"), 10, 5).unwrap();
        assert_eq!(got, [past("two", 5), past("one", 2)]);
    }

    #[test]
    fn only_the_newest_up_to_the_limit_stay() {
        let stored = [past("b", 2), past("a", 1)];
        assert_eq!(carry(Some(&stored), Some("c"), Some("d"), 2, 5).unwrap(), [past("c", 5), past("b", 2)]);
        // Off: the next save drops what was kept.
        assert_eq!(carry(Some(&stored), Some("c"), Some("d"), 0, 5), None);
    }

    #[test]
    fn the_limit_setting_reads_as_set() {
        assert_eq!(parse_limit(None), DEFAULT_LIMIT);
        assert_eq!(parse_limit(Some("garbage")), DEFAULT_LIMIT);
        assert_eq!(parse_limit(Some("0")), 0);
        assert_eq!(parse_limit(Some(" 25 ")), 25);
        assert_eq!(parse_limit(Some("5000")), MAX_LIMIT);
    }

    #[test]
    fn only_typed_passwords_count() {
        assert_eq!(typed_password(&AuthMethod::Password { password: Some("x".into()), password_ref: None }), Some("x"));
        assert_eq!(typed_password(&AuthMethod::Password { password: Some(String::new()), password_ref: None }), None);
        assert_eq!(typed_password(&AuthMethod::Agent), None);
    }

    #[test]
    fn the_history_is_not_written_when_empty() {
        let s: crate::state::SessionConfig = serde_json::from_value(serde_json::json!({
            "id": "a", "name": "a", "host": "h", "port": 22, "username": "u",
            "auth_method": { "type": "Agent" }, "folder_id": null, "tags": [],
        }))
        .unwrap();
        assert!(s.password_history.is_none());
        assert!(serde_json::to_value(&s).unwrap().get("password_history").is_none());
    }
}
