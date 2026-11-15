//! A record as it travels to a sync backend: one secret, or its deletion,
//! with its version, sealed so the backend learns nothing from it.
//!
//! Everything about the secret is inside the seal: its name, its category,
//! its timestamps and the payload, which is itself still encrypted under its
//! own key as in the local file. The vault id and the record id are bound in
//! as associated data, so a backend cannot move one record's seal into
//! another's place or into another vault. The id a backend files a record
//! under is a keyed hash of the real one, so ids such as a setting's key do
//! not show either. The version inside lets a device
//! refuse an older copy of a record it has already seen.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::vault::error::VaultError;
use crate::vault::sync_local::Row;
use crate::vault::types::Dek;

const FORMAT: u8 = 1;

/// One record: a secret's row, or `None` once deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub id: String,
    /// Milliseconds since the epoch of the change, made to grow per record.
    pub version: i64,
    /// The device that made the change; breaks a tie between versions.
    pub device: String,
    pub row: Option<Row>,
}

impl Record {
    /// Whether this record is newer than a change made at `version` on
    /// `device`.
    pub fn newer_than(&self, version: i64, device: &str) -> bool {
        (self.version, self.device.as_str()) > (version, device)
    }
}

#[derive(Serialize, Deserialize)]
struct Sealed {
    id: String,
    version: i64,
    device: String,
    deleted: bool,
    #[serde(default)]
    name: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    nonce: String,
    #[serde(default)]
    ciphertext: String,
    #[serde(default)]
    wrapped_dek: String,
    #[serde(default)]
    created_at: i64,
    #[serde(default)]
    updated_at: i64,
}

/// The key that seals a vault's records, derived from its master key.
pub struct RecordKey {
    key: Zeroizing<[u8; 32]>,
    vault_id: String,
}

impl RecordKey {
    pub fn derive(master: &Dek, vault_id: &str) -> Result<Self, VaultError> {
        let mut key = Zeroizing::new([0u8; 32]);
        master
            .with_key(|m| Hkdf::<Sha256>::new(None, m).expand(b"reach-sync-record-v1", key.as_mut()))
            .map_err(|_| VaultError::CryptoError("record key derivation failed".into()))?;
        Ok(Self { key, vault_id: vault_id.to_string() })
    }

    /// The same key again, for a second user of it.
    pub fn duplicate(&self) -> Self {
        Self { key: self.key.clone(), vault_id: self.vault_id.clone() }
    }

    /// A key of its own for something else derived from the vault, such as a
    /// bundle file.
    pub fn subkey(&self, label: &str) -> Result<Zeroizing<[u8; 32]>, VaultError> {
        let mut key = Zeroizing::new([0u8; 32]);
        Hkdf::<Sha256>::new(None, self.key.as_ref())
            .expand(label.as_bytes(), key.as_mut())
            .map_err(|_| VaultError::CryptoError("key derivation failed".into()))?;
        Ok(key)
    }

    pub fn vault_id(&self) -> &str {
        &self.vault_id
    }

    /// The id a backend files a record under: a keyed hash of its own id.
    pub fn remote_id(&self, id: &str) -> String {
        // HKDF-Expand is a keyed pseudo-random function, so its output for
        // `id` reveals nothing of `id` without the key.
        let mut out = [0u8; 16];
        Hkdf::<Sha256>::new(Some(b"reach-sync-id-v1"), self.key.as_ref())
            .expand_multi_info(&[self.vault_id.as_bytes(), b"\0", id.as_bytes()], &mut out)
            .expect("16 bytes is a valid HKDF length");
        out.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn aad(&self, remote_id: &str) -> Vec<u8> {
        let mut aad = vec![FORMAT];
        aad.extend_from_slice(self.vault_id.as_bytes());
        aad.push(0);
        aad.extend_from_slice(remote_id.as_bytes());
        aad
    }

    /// Seal a record: a format byte, a random nonce, then the ciphertext.
    pub fn seal(&self, record: &Record) -> Result<Vec<u8>, VaultError> {
        let sealed = match &record.row {
            Some(r) => Sealed {
                id: record.id.clone(),
                version: record.version,
                device: record.device.clone(),
                deleted: false,
                name: r.name.clone(),
                category: r.category.clone(),
                nonce: BASE64.encode(&r.nonce),
                ciphertext: BASE64.encode(&r.ciphertext),
                wrapped_dek: r.wrapped_dek.clone(),
                created_at: r.created_at,
                updated_at: r.updated_at,
            },
            None => Sealed {
                id: record.id.clone(),
                version: record.version,
                device: record.device.clone(),
                deleted: true,
                name: String::new(),
                category: String::new(),
                nonce: String::new(),
                ciphertext: String::new(),
                wrapped_dek: String::new(),
                created_at: 0,
                updated_at: 0,
            },
        };
        let plain = Zeroizing::new(serde_json::to_vec(&sealed)?);
        let nonce: [u8; 24] = rand::random();
        let ct = XChaCha20Poly1305::new((&*self.key).into())
            .encrypt(&XNonce::from(nonce), Payload { msg: &plain, aad: &self.aad(&self.remote_id(&record.id)) })
            .map_err(|e| VaultError::EncryptionError(e.to_string()))?;
        let mut out = Vec::with_capacity(1 + 24 + ct.len());
        out.push(FORMAT);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    /// Open a sealed record a backend filed under `remote_id`.
    pub fn open(&self, remote_id: &str, data: &[u8]) -> Result<Record, VaultError> {
        let id = remote_id;
        if data.len() < 1 + 24 || data[0] != FORMAT {
            return Err(VaultError::DecryptionError(format!("record {id} is not in a known format")));
        }
        let nonce: [u8; 24] = data[1..25].try_into().expect("checked length");
        let plain = Zeroizing::new(
            XChaCha20Poly1305::new((&*self.key).into())
                .decrypt(&XNonce::from(nonce), Payload { msg: &data[25..], aad: &self.aad(id) })
                .map_err(|_| VaultError::DecryptionError(format!("record {id} does not open with this vault's key")))?,
        );
        let s: Sealed = serde_json::from_slice(&plain)?;
        if self.remote_id(&s.id) != remote_id {
            return Err(VaultError::DecryptionError(format!("record {remote_id} is filed under the wrong id")));
        }
        let id = s.id.as_str();
        let decode = |v: &str| BASE64.decode(v).map_err(|e| VaultError::SerializationError(e.to_string()));
        let row = if s.deleted {
            None
        } else {
            Some(Row {
                id: id.to_string(),
                name: s.name,
                category: s.category,
                nonce: decode(&s.nonce)?,
                ciphertext: decode(&s.ciphertext)?,
                wrapped_dek: s.wrapped_dek,
                created_at: s.created_at,
                updated_at: s.updated_at,
            })
        };
        Ok(Record { id: s.id.clone(), version: s.version, device: s.device, row })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(vault: &str) -> RecordKey {
        RecordKey::derive(&Dek::from_fn(|k| k.fill(9)), vault).unwrap()
    }

    fn record() -> Record {
        Record {
            id: "s1".into(),
            version: 1_700_000_000_000,
            device: "laptop".into(),
            row: Some(Row {
                id: "s1".into(),
                name: "prod-db.example.com".into(),
                category: "session".into(),
                nonce: vec![4; 24],
                ciphertext: vec![5, 6, 7],
                wrapped_dek: "{\"nonce\":[]}".into(),
                created_at: 10,
                updated_at: 11,
            }),
        }
    }

    #[test]
    fn a_record_comes_back_as_it_went_in() {
        let k = key("v1");
        let sealed = k.seal(&record()).unwrap();
        assert_eq!(k.open(&k.remote_id("s1"), &sealed).unwrap(), record());
        let gone = Record { row: None, ..record() };
        assert_eq!(k.open(&k.remote_id("s1"), &k.seal(&gone).unwrap()).unwrap(), gone);
    }

    #[test]
    fn the_name_is_not_readable() {
        let sealed = key("v1").seal(&record()).unwrap();
        assert!(!sealed.windows(7).any(|w| w == b"prod-db"));
        assert!(!sealed.windows(7).any(|w| w == b"session"));
    }

    #[test]
    fn a_record_cannot_move_to_another_id_or_vault() {
        let sealed = key("v1").seal(&record()).unwrap();
        assert!(key("v1").open(&key("v1").remote_id("s2"), &sealed).is_err());
        assert!(key("v2").open(&key("v2").remote_id("s1"), &sealed).is_err());
        assert!(key("v1").open(&key("v1").remote_id("s1"), &sealed).is_ok());
    }

    #[test]
    fn remote_ids_hide_the_real_ones() {
        let k = key("v1");
        assert_eq!(k.remote_id("app_settings").len(), 32);
        assert_ne!(k.remote_id("app_settings"), key("v2").remote_id("app_settings"));
        assert_eq!(k.remote_id("x"), k.remote_id("x"));
    }

    #[test]
    fn newer_compares_version_then_device() {
        let r = record();
        assert!(r.newer_than(r.version - 1, "zzz"));
        assert!(!r.newer_than(r.version + 1, "aaa"));
        assert!(r.newer_than(r.version, "aaa"));
        assert!(!r.newer_than(r.version, "laptop"));
    }
}
