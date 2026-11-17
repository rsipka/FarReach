//! Key passphrases kept in memory for this run, so a passphrase-protected
//! key is asked for once, not at every connect.
//!
//! Off unless Settings → Security → "Unlock credential keys at start" is
//! on. Then the keys of the saved credentials are asked for right after the
//! vault opens (`saved_credential_commands::credential_keys_unlock`), and a
//! passphrase typed at a connect is kept too. Nothing is written anywhere:
//! the passphrases go when the vault locks, when the setting is turned off
//! and when the app quits. A passphrase is kept by the hash of the key it
//! opens, so the same key reached another way (a credential, a session's own
//! key, an IdentityFile) is open as well.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

type KeyHash = [u8; 32];

static ENABLED: AtomicBool = AtomicBool::new(false);

fn kept() -> &'static Mutex<HashMap<KeyHash, Zeroizing<String>>> {
    static K: OnceLock<Mutex<HashMap<KeyHash, Zeroizing<String>>>> = OnceLock::new();
    K.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Which key this is, whatever line ends or byte order mark its text has.
pub(crate) fn hash(material: &str) -> KeyHash {
    Sha256::digest(super::client::normalize_key_text(material).as_bytes()).into()
}

pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// Turning it off forgets what was kept.
pub(crate) fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
    if !on {
        forget();
    }
}

/// The passphrase kept for this key, if any.
pub(crate) fn get(material: &str) -> Option<String> {
    kept().lock().unwrap().get(&hash(material)).map(|p| p.as_str().to_string())
}

/// Keep a passphrase that has just opened this key. Does nothing while the
/// setting is off.
pub(crate) fn remember(material: &str, passphrase: &str) {
    if enabled() && !passphrase.is_empty() {
        kept().lock().unwrap().insert(hash(material), Zeroizing::new(passphrase.to_string()));
    }
}

/// The vault locked, the setting went off, or the app is quitting.
pub fn forget() {
    kept().lock().unwrap().clear();
}

/// Whether the key can be used as it is: read with the passphrase stored
/// with it (or none) or with one kept here.
pub(crate) fn opens(material: &str, stored: Option<&str>) -> bool {
    let open = |pass: Option<&str>| super::client::decode_key(material, pass).is_ok_and(|k| !k.is_encrypted());
    open(stored) || get(material).is_some_and(|p| open(Some(&p)))
}

/// The setting and the kept passphrases are process-wide: tests that touch
/// them take this first.
#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    // The throwaway keys of the keystore tests.
    const PLAIN: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\nQyNTUxOQAAACCwsCZ1pFDo+/0R+iXN9Emzi+emmN0GEbCoIL9hE1eafgAAAJhbzNn6W8zZ\n+gAAAAtzc2gtZWQyNTUxOQAAACCwsCZ1pFDo+/0R+iXN9Emzi+emmN0GEbCoIL9hE1eafg\nAAAEDs2ThleIPEbrfMS5+KvabXLE0113+oUG6NIEaDtPChvLCwJnWkUOj7/RH6Jc30SbOL\n56aY3QYRsKggv2ETV5p+AAAAEnJlYWNoLXRlc3RAZXhhbXBsZQECAw==\n-----END OPENSSH PRIVATE KEY-----\n";
    const ENCRYPTED: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jdHIAAAAGYmNyeXB0AAAAGAAAABCcqk0FV4\nKt6ydcd+Fwf0h1AAAAGAAAAAEAAAAzAAAAC3NzaC1lZDI1NTE5AAAAIJk+zQU8O+k3l0Y9\nx4epBs/36Jir8BpbhqTJH0+tr864AAAAoPYA20IkAMre5HEKk0iDCkEk72vCMulAvfM7LK\nO3hbWbXJAzOgl5v1AzwoTzi73AvzN7rTxU+wsQ9vuWntsc7WjaqdQeg/ZHi3pNKRkyX+Ay\nGmXQTdgeHjjkQ/mc7GTjpqYFV32+LlFRoC+zP50ivDKINyUXgzd0zz/G2xR5zQjU/boKQU\n719oRMkiGz8P1eib8piYDdiJ3fY0MDqbac8QM=\n-----END OPENSSH PRIVATE KEY-----\n";
    const PASSPHRASE: &str = "hunter2";

    // One test: the setting and the kept passphrases are process-wide.
    #[test]
    fn keeps_a_passphrase_only_while_on_and_until_forgotten() {
        let _alone = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_enabled(false);
        remember(ENCRYPTED, PASSPHRASE);
        assert_eq!(get(ENCRYPTED), None, "nothing is kept while the setting is off");
        assert!(opens(PLAIN, None));
        assert!(!opens(ENCRYPTED, None));
        assert!(opens(ENCRYPTED, Some(PASSPHRASE)));

        set_enabled(true);
        remember(ENCRYPTED, "");
        assert_eq!(get(ENCRYPTED), None);
        remember(ENCRYPTED, PASSPHRASE);
        // The same key with Windows line ends and a byte order mark.
        let messy = format!("\u{feff}{}", ENCRYPTED.replace('\n', "\r\n"));
        assert_eq!(get(&messy).as_deref(), Some(PASSPHRASE));
        assert!(opens(ENCRYPTED, None));
        assert_eq!(get(PLAIN), None);

        forget();
        assert!(!opens(ENCRYPTED, None));
        remember(ENCRYPTED, PASSPHRASE);
        set_enabled(false);
        assert_eq!(get(ENCRYPTED), None, "turning the setting off forgets");
    }
}
