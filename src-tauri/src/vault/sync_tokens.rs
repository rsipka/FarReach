//! The sync tokens in the identity file, sealed.
//!
//! A Turso database token is a bearer credential: whoever holds it reads and
//! writes that database. They used to sit in `vault_identity.json` in the
//! clear. They are now kept there as one sealed value, XChaCha20-Poly1305
//! under a key derived from the identity's secret key, so the file alone gives
//! nothing away; the key exists only while Reach is unlocked, which is the
//! only time a token is needed.
//!
//! A file from an older version still has its tokens in the clear. They are
//! read as before and sealed on the next save.

use std::collections::HashMap;

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

const FORMAT: u8 = 1;

/// Every token the identity file holds: the personal sync token, and one per
/// user vault, by vault id.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncTokens {
    #[serde(default)]
    pub personal: Option<String>,
    #[serde(default)]
    pub vaults: HashMap<String, String>,
}

impl SyncTokens {
    pub fn is_empty(&self) -> bool {
        self.personal.is_none() && self.vaults.is_empty()
    }
}

fn key(identity_secret: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let mut key = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(None, identity_secret)
        .expand(format!("reach-sync-tokens-v{FORMAT}").as_bytes(), key.as_mut())
        .map_err(|_| VaultError::CryptoError("sync token key derivation failed".into()))?;
    Ok(key)
}

/// Bound in as associated data, so one user's sealed tokens cannot be passed
/// off as another's.
fn aad(user_uuid: &str) -> Vec<u8> {
    let mut aad = vec![FORMAT];
    aad.extend_from_slice(user_uuid.as_bytes());
    aad
}

/// Seal the tokens: base64 of a format byte, a random nonce, the ciphertext.
pub fn seal(identity_secret: &[u8; 32], user_uuid: &str, tokens: &SyncTokens) -> Result<String, VaultError> {
    let key = key(identity_secret)?;
    let plain = Zeroizing::new(serde_json::to_vec(tokens)?);
    let nonce: [u8; 24] = rand::random();
    let sealed = XChaCha20Poly1305::new((&*key).into())
        .encrypt(&XNonce::from(nonce), Payload { msg: &plain, aad: &aad(user_uuid) })
        .map_err(|e| VaultError::EncryptionError(e.to_string()))?;
    let mut out = Vec::with_capacity(1 + 24 + sealed.len());
    out.push(FORMAT);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(BASE64.encode(out))
}

/// Open sealed tokens.
pub fn open(identity_secret: &[u8; 32], user_uuid: &str, sealed: &str) -> Result<SyncTokens, VaultError> {
    let data = BASE64
        .decode(sealed)
        .map_err(|e| VaultError::SerializationError(e.to_string()))?;
    if data.len() < 1 + 24 || data[0] != FORMAT {
        return Err(VaultError::DecryptionError("unknown sync token format".into()));
    }
    let key = key(identity_secret)?;
    let nonce: [u8; 24] = data[1..25].try_into().expect("checked length");
    let plain = Zeroizing::new(
        XChaCha20Poly1305::new((&*key).into())
            .decrypt(&XNonce::from(nonce), Payload { msg: &data[25..], aad: &aad(user_uuid) })
            .map_err(|e| VaultError::DecryptionError(e.to_string()))?,
    );
    Ok(serde_json::from_slice(&plain)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> SyncTokens {
        SyncTokens {
            personal: Some("eyJ.personal".into()),
            vaults: HashMap::from([("v1".to_string(), "eyJ.shared".to_string())]),
        }
    }

    #[test]
    fn tokens_come_back_as_they_went_in() {
        let sealed = seal(&[7; 32], "user-a", &tokens()).unwrap();
        assert_eq!(open(&[7; 32], "user-a", &sealed).unwrap(), tokens());
    }

    #[test]
    fn the_sealed_value_does_not_show_a_token() {
        let sealed = seal(&[7; 32], "user-a", &tokens()).unwrap();
        let raw = BASE64.decode(&sealed).unwrap();
        assert!(!sealed.contains("eyJ"));
        assert!(!raw.windows(3).any(|w| w == b"eyJ"));
    }

    #[test]
    fn another_key_or_user_cannot_open_it() {
        let sealed = seal(&[7; 32], "user-a", &tokens()).unwrap();
        assert!(open(&[8; 32], "user-a", &sealed).is_err());
        assert!(open(&[7; 32], "user-b", &sealed).is_err());
    }
}
