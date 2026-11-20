//! Key passphrases remembered behind the phone's biometric (Settings →
//! Security → "Remember key passphrases with …", under "Unlock credential
//! keys at start"), so one Face ID or fingerprint check at start opens every
//! credential key instead of typing each passphrase.
//!
//! The passphrases never go into the vault and never sync: they are one
//! blob on this device that only the biometric opens.
//!
//! - Android: an AES-256-GCM key in the Android Keystore that needs a strong
//!   biometric for every use, and dies when a fingerprint is enrolled, seals
//!   the blob; this file keeps its IV and ciphertext. The same Kotlin code as
//!   the vault's fingerprint unlock, under its own alias.
//! - iOS: the blob is a keychain item readable only after Face ID or Touch ID
//!   (current enrolled set, this device only, [`crate::vault::apple_biometric`]);
//!   this file only records that it is there, so the start does not ask
//!   Face ID for nothing.
//! - Windows and macOS: the file holds an encrypted blob. An independent
//!   Windows Hello credential or a Touch ID-protected keychain item opens it.
//!
//! Each passphrase is kept under the key's names from
//! [`super::unlocked::key_ids`] (public key fingerprint and text hash), so a
//! passphrase opens the same key wherever it is stored. What it opens goes
//! to [`super::unlocked`] for this run, as a typed passphrase does.

use std::path::Path;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

const FILE: &str = "key_passphrases.json";

/// Android Keystore alias of the sealing key.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const ANDROID_ALIAS: &str = "reach-key-passphrases";

/// iOS keychain item of the blob.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
const IOS_SERVICE: &str = "reachercm-key-passphrases";
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
const IOS_ACCOUNT: &str = "passphrases";

/// Whether this build can keep passphrases behind a biometric.
pub fn supported() -> bool {
    cfg!(any(windows, target_os = "macos", target_os = "android", target_os = "ios"))
}

/// Whether this build has access to the protected store in addition to
/// having the platform biometric configured.
pub async fn storage_available() -> bool {
    #[cfg(target_os = "macos")]
    return tokio::task::spawn_blocking(platform::storage_available).await.unwrap_or(false);
    #[cfg(not(target_os = "macos"))]
    supported()
}

/// One key's passphrase and the names of that key.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Entry {
    pub ids: Vec<String>,
    pub passphrase: String,
}

impl Drop for Entry {
    fn drop(&mut self) {
        self.passphrase.zeroize();
    }
}

/// What is kept in [`FILE`].
#[derive(Serialize, Deserialize, Default)]
struct Saved {
    #[serde(default)]
    iv: String,
    #[serde(default)]
    ciphertext: String,
    #[serde(default)]
    count: usize,
    /// Windows Hello signs this challenge to derive the encryption key.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    challenge: String,
}

#[cfg(any(windows, target_os = "macos", test))]
mod desktop_seal {
    use super::{Entry, Saved};
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use chacha20poly1305::{
        aead::{Aead, KeyInit, Payload},
        XChaCha20Poly1305, XNonce,
    };
    use zeroize::Zeroizing;

    const PURPOSE: &[u8] = b"Reacher CM SSH key passphrases v1";

    pub(super) fn open(saved: Saved, key: &[u8; 32]) -> Result<Vec<Entry>, String> {
        let nonce: [u8; 24] = BASE64
            .decode(saved.iv)
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|_| "The remembered passphrases have an invalid nonce")?;
        let ciphertext = BASE64.decode(saved.ciphertext).map_err(|e| e.to_string())?;
        let bytes = XChaCha20Poly1305::new(key.into())
            .decrypt(&XNonce::from(nonce), Payload { msg: &ciphertext, aad: PURPOSE })
            .map(Zeroizing::new)
            .map_err(|_| "The remembered passphrases could not be decrypted")?;
        super::decode(&bytes)
    }

    pub(super) fn seal(bytes: &[u8], count: usize, challenge: &[u8; 32], key: &[u8; 32]) -> Result<Saved, String> {
        let nonce: [u8; 24] = rand::random();
        let ciphertext = XChaCha20Poly1305::new(key.into())
            .encrypt(&XNonce::from(nonce), Payload { msg: bytes, aad: PURPOSE })
            .map_err(|_| "Could not encrypt the passphrases")?;
        Ok(Saved {
            iv: BASE64.encode(nonce),
            ciphertext: BASE64.encode(ciphertext),
            count,
            challenge: BASE64.encode(challenge),
        })
    }
}

fn load(dir: &Path) -> Option<Saved> {
    serde_json::from_slice(&std::fs::read(dir.join(FILE)).ok()?).ok()
}

/// Whether passphrases are remembered on this device.
pub fn exists(dir: &Path) -> bool {
    load(dir).is_some_and(|s| s.count > 0)
}

/// `old` with `new` added: an entry of `new` replaces every old one that
/// names the same key (its passphrase was changed).
pub fn merge(old: Vec<Entry>, new: Vec<Entry>) -> Vec<Entry> {
    let mut out: Vec<Entry> = old
        .into_iter()
        .filter(|o| !new.iter().any(|n| n.ids.iter().any(|id| o.ids.contains(id))))
        .collect();
    out.extend(new);
    out
}

fn encode(entries: &[Entry]) -> Result<Zeroizing<Vec<u8>>, String> {
    serde_json::to_vec(entries).map(Zeroizing::new).map_err(|e| e.to_string())
}

fn decode(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    serde_json::from_slice(bytes).map_err(|_| "The remembered passphrases are damaged".to_string())
}

/// Why opening failed.
#[derive(Debug)]
pub enum OpenError {
    /// The biometric changed or the key is gone: what was kept is void and
    /// has been removed. The passphrases are typed again.
    Gone,
    /// Cancelled, or the check failed; nothing changed.
    Declined(String),
}

#[cfg(any(test, not(any(windows, target_os = "macos"))))]
fn forget_file(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(FILE));
}

fn write_file(dir: &Path, saved: &Saved) -> Result<(), String> {
    let data = serde_json::to_vec_pretty(saved).map_err(|e| e.to_string())?;
    crate::vault::private_fs::write(&dir.join(FILE), &data).map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
mod platform {
    use super::*;
    use crate::ipc::vault_commands::PromptText;
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use serde_json::json;
    use tauri::{AppHandle, Manager};
    use tauri_plugin_reach_unlock::Unlock;

    fn plugin(app: &AppHandle) -> tauri::State<'_, Unlock<tauri::Wry>> {
        app.state::<Unlock<tauri::Wry>>()
    }

    #[derive(Deserialize)]
    struct Present {
        present: bool,
    }

    #[derive(Deserialize)]
    struct Sealed {
        iv: String,
        ciphertext: String,
    }

    #[derive(Deserialize)]
    struct Opened {
        secret: String,
    }

    pub async fn open(app: &AppHandle, dir: &Path, prompt: &PromptText) -> Result<Vec<Entry>, OpenError> {
        let saved = load(dir).ok_or(OpenError::Gone)?;
        let present = plugin(app)
            .call::<Present>("biometricHasKey", json!({ "alias": ANDROID_ALIAS }))
            .await
            .map(|p| p.present)
            .unwrap_or(true);
        if !present {
            forget_file(dir);
            return Err(OpenError::Gone);
        }
        let payload = json!({
            "alias": ANDROID_ALIAS,
            "iv": saved.iv,
            "ciphertext": saved.ciphertext,
            "title": prompt.title,
            "subtitle": prompt.subtitle,
            "cancel": prompt.cancel,
        });
        let opened: Opened = plugin(app).call("biometricOpen", payload).await.map_err(|e| {
            // A fingerprint enrolled since: the Keystore voided the key (and
            // the plugin removed it). Anything else is a cancel or a miss.
            if e.contains("invalidated") || e.contains("fingerprints changed") || e.contains("is gone") {
                forget_file(dir);
                OpenError::Gone
            } else {
                OpenError::Declined(e)
            }
        })?;
        let bytes = Zeroizing::new(BASE64.decode(&opened.secret).map_err(|e| OpenError::Declined(e.to_string()))?);
        decode(&bytes).map_err(OpenError::Declined)
    }

    pub async fn save(app: &AppHandle, dir: &Path, entries: &[Entry], prompt: &PromptText) -> Result<(), String> {
        let blob = encode(entries)?;
        let payload = json!({
            "alias": ANDROID_ALIAS,
            "secret": BASE64.encode(&*blob),
            "title": prompt.title,
            "subtitle": prompt.subtitle,
            "cancel": prompt.cancel,
        });
        let sealed: Sealed = plugin(app).call("biometricSeal", payload).await?;
        write_file(dir, &Saved { iv: sealed.iv, ciphertext: sealed.ciphertext, count: entries.len(), ..Default::default() })
    }

    pub async fn forget(app: &AppHandle, dir: &Path) -> Result<(), String> {
        let _ = plugin(app).call::<serde_json::Value>("biometricForget", json!({ "alias": ANDROID_ALIAS })).await;
        forget_file(dir);
        Ok(())
    }
}

#[cfg(any(windows, target_os = "macos"))]
#[path = "biometric_keys_desktop.rs"]
mod platform;

#[cfg(not(any(windows, target_os = "macos", target_os = "android")))]
mod platform {
    use super::*;
    use crate::ipc::vault_commands::PromptText;
    use crate::vault::apple_biometric::{self, ReadError};
    use tauri::AppHandle;

    /// iOS shows its own prompt (the app's Face ID usage text); `prompt` is
    /// for Android's.
    pub async fn open(_app: &AppHandle, dir: &Path, _prompt: &PromptText) -> Result<Vec<Entry>, OpenError> {
        if !exists(dir) {
            return Err(OpenError::Gone);
        }
        let read = tokio::task::spawn_blocking(|| apple_biometric::read(IOS_SERVICE, IOS_ACCOUNT))
            .await
            .map_err(|e| OpenError::Declined(e.to_string()))?;
        match read {
            Ok(bytes) => decode(&bytes).map_err(OpenError::Declined),
            Err(ReadError::Gone) => {
                forget_file(dir);
                Err(OpenError::Gone)
            }
            Err(ReadError::Declined(e)) => Err(OpenError::Declined(e)),
        }
    }

    /// Writing the item asks nothing on iOS.
    pub async fn save(_app: &AppHandle, dir: &Path, entries: &[Entry], _prompt: &PromptText) -> Result<(), String> {
        if !supported() {
            return Err("This device cannot remember passphrases with a biometric".into());
        }
        let blob = encode(entries)?;
        tokio::task::spawn_blocking(move || apple_biometric::store(IOS_SERVICE, IOS_ACCOUNT, &blob))
            .await
            .map_err(|e| e.to_string())??;
        write_file(dir, &Saved { count: entries.len(), ..Default::default() })
    }

    pub async fn forget(_app: &AppHandle, dir: &Path) -> Result<(), String> {
        let _ = tokio::task::spawn_blocking(|| apple_biometric::delete(IOS_SERVICE, IOS_ACCOUNT)).await;
        forget_file(dir);
        Ok(())
    }
}

pub use platform::{forget, open, save};

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(ids: &[&str], pass: &str) -> Entry {
        Entry { ids: ids.iter().map(|s| s.to_string()).collect(), passphrase: pass.into() }
    }

    #[test]
    fn a_new_passphrase_replaces_the_old_one_of_the_same_key() {
        let old = vec![entry(&["fp-a", "text-a"], "old-a"), entry(&["fp-b"], "b")];
        let new = vec![entry(&["fp-a", "text-a2"], "new-a"), entry(&["text-c"], "c")];
        let merged = merge(old, new);
        assert!(merged == vec![entry(&["fp-b"], "b"), entry(&["fp-a", "text-a2"], "new-a"), entry(&["text-c"], "c")]);
    }

    #[test]
    fn entries_round_trip_and_damage_is_reported() {
        let entries = vec![entry(&["x"], "pass")];
        assert!(decode(&encode(&entries).unwrap()).unwrap() == entries);
        assert!(decode(b"not json").is_err());
    }

    #[test]
    fn desktop_blob_can_only_be_opened_with_its_wrapping_key() {
        let entries = vec![entry(&["fingerprint"], "test passphrase")];
        let bytes = encode(&entries).unwrap();
        let key: [u8; 32] = rand::random();
        let challenge: [u8; 32] = rand::random();
        let saved = desktop_seal::seal(&bytes, entries.len(), &challenge, &key).unwrap();
        let mut wrong_key = key;
        wrong_key[0] ^= 1;
        let stored = serde_json::to_vec(&saved).unwrap();
        assert!(desktop_seal::open(serde_json::from_slice(&stored).unwrap(), &wrong_key).is_err());
        assert!(desktop_seal::open(saved, &key).unwrap() == entries);
    }

    #[test]
    fn the_file_says_whether_anything_is_remembered() {
        let dir = std::env::temp_dir().join(format!("reach-bio-keys-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!exists(&dir));
        write_file(&dir, &Saved { count: 2, ..Default::default() }).unwrap();
        assert!(exists(&dir));
        forget_file(&dir);
        assert!(!exists(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
