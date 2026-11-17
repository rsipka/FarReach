//! Linking a second device to the same vaults.
//!
//! Every vault key is wrapped for one identity: its user id, X25519 key and
//! the salt its key-encryption key is derived with. A device that set up an
//! identity of its own can reach the same sync database but never open the
//! vault in it, so it made one of its own. A link code carries the first
//! device's identity and sync settings to the second, which then opens the
//! very vaults the first one syncs, as another device of the same user.

use super::*;

/// What a link code starts with, so a plain backup key is told apart.
pub const LINK_PREFIX: &str = "reachercm-link1:";

#[derive(Serialize, Deserialize)]
struct DeviceLink {
    user_uuid: String,
    salt: String,
    public_key: String,
    secret_key: String,
    /// The master password's copy of the key, so the same password opens
    /// the linked device too. Empty when no password is set.
    #[serde(default)]
    encrypted_key: String,
    #[serde(default)]
    nonce: String,
    personal_sync_url: Option<String>,
    personal_sync_token: Option<String>,
    #[serde(default)]
    internal_vault_ids: HashMap<String, String>,
    /// Only the vaults that sync: a local one has nothing to open elsewhere.
    #[serde(default)]
    user_vaults: Vec<StoredVaultRef>,
}

/// Whether `text` is a link code rather than a plain backup key.
pub fn is_link_code(text: &str) -> bool {
    text.trim().starts_with(LINK_PREFIX)
}

fn parse(code: &str) -> Result<DeviceLink, VaultError> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let body = code
        .trim()
        .strip_prefix(LINK_PREFIX)
        .ok_or_else(|| VaultError::SerializationError("This is not a device link code".into()))?;
    // A code pasted from a chat may have been wrapped across lines.
    let body: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    let json = URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|_| VaultError::SerializationError("The device link code is incomplete or damaged".into()))?;
    serde_json::from_slice(&json)
        .map_err(|_| VaultError::SerializationError("The device link code is incomplete or damaged".into()))
}

impl VaultManager {
    /// A code that links another device to this identity and the vaults it
    /// syncs. It holds the identity's secret key and the sync tokens, so it
    /// is as sensitive as the backup key.
    pub async fn export_device_link(&self) -> Result<String, VaultError> {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let identity = self.identity.as_ref().ok_or(VaultError::Locked)?;
        let user_uuid = self.user_uuid.clone().ok_or(VaultError::Locked)?;
        let data = tokio::fs::read_to_string(self.app_dir.join("vault_identity.json")).await?;
        let stored: StoredIdentity = serde_json::from_str(&data)?;

        let user_vaults: Vec<StoredVaultRef> = self
            .user_vaults
            .iter()
            .filter(|v| v.sync_url.is_some() && v.sync_token.is_some())
            .cloned()
            .collect();
        let has_personal = self.personal_sync_url.is_some() && self.personal_sync_token.is_some();
        if !has_personal && user_vaults.is_empty() {
            return Err(VaultError::SyncError(
                "Nothing syncs yet. Turn on sync on this device first, then link the other one.".into(),
            ));
        }
        // Only the synced personal vault exists anywhere but here.
        let internal_vault_ids = match (has_personal, self.internal_vault_ids.get("__personal__")) {
            (true, Some(id)) => self
                .internal_vault_ids
                .iter()
                .filter(|(_, v)| *v == id)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            _ => HashMap::new(),
        };

        let link = DeviceLink {
            user_uuid,
            salt: stored.salt,
            public_key: BASE64.encode(identity.public_key.as_bytes()),
            secret_key: identity.with_secret(|s| BASE64.encode(s.as_bytes())),
            encrypted_key: stored.encrypted_key,
            nonce: stored.nonce,
            personal_sync_url: self.personal_sync_url.clone().filter(|_| has_personal),
            personal_sync_token: self.personal_sync_token.clone().filter(|_| has_personal),
            internal_vault_ids,
            user_vaults,
        };
        let json = zeroize::Zeroizing::new(serde_json::to_vec(&link)?);
        Ok(format!("{}{}", LINK_PREFIX, URL_SAFE_NO_PAD.encode(&*json)))
    }

    /// Make this device another device of the identity in `code` and open
    /// the vaults it syncs. Whatever identity this device had is replaced:
    /// its key stays in the OS keychain and its files stay on disk, but it
    /// no longer opens.
    pub async fn link_device(&mut self, code: &str) -> Result<String, VaultError> {
        let link = parse(code)?;
        let secret = zeroize::Zeroizing::new(
            BASE64
                .decode(&link.secret_key)
                .map_err(|e| VaultError::SerializationError(e.to_string()))?,
        );
        if secret.len() != 32 {
            return Err(VaultError::InvalidKeyLength { expected: 32, got: secret.len() });
        }
        let mut sk = zeroize::Zeroizing::new([0u8; 32]);
        sk.copy_from_slice(&secret);
        let public = PublicKey::from(&StaticSecret::from(*sk));
        if BASE64.encode(public.as_bytes()) != link.public_key {
            return Err(VaultError::SerializationError("The device link code is incomplete or damaged".into()));
        }
        if let Some(url) = link.personal_sync_url.as_deref() {
            crate::vault::sync::check_sync_url(url)?;
        }

        // Forget the identity this device had.
        self.vaults.clear();
        self.vault_names.clear();
        self.kek = None;
        self.identity = None;
        self.user_uuid = None;
        self.identity_public_key = None;
        self.personal_sync_url = None;
        self.personal_sync_token = None;
        self.internal_vault_ids.clear();
        self.user_vaults.clear();
        self.device_id = None;

        if let Err(e) = store_key_in_keychain(&link.user_uuid, &*sk) {
            tracing::warn!("Linked without the OS keychain ({e}); the master password opens it");
        }

        // The file never holds a token in the clear: it is written without
        // them, and the unlock below seals them into it.
        let stored = StoredIdentity {
            user_uuid: link.user_uuid.clone(),
            salt: link.salt,
            encrypted_key: link.encrypted_key,
            nonce: link.nonce,
            public_key: link.public_key,
            personal_sync_url: link.personal_sync_url,
            personal_sync_token: None,
            internal_vault_ids: link.internal_vault_ids,
            user_vaults: link.user_vaults.iter().map(|v| StoredVaultRef { sync_token: None, ..v.clone() }).collect(),
            sealed_tokens: None,
            // This is another device of the same user.
            device_id: None,
        };
        private_fs::private_dir(&self.app_dir)?;
        let path = self.app_dir.join("vault_identity.json");
        private_fs::write_async(&path, serde_json::to_string_pretty(&stored)?.into_bytes()).await?;

        let with_tokens = StoredIdentity {
            personal_sync_token: link.personal_sync_token,
            user_vaults: link.user_vaults,
            ..stored
        };
        self.open_with_secret_key(with_tokens, &*sk).await?;
        self.held = false;
        Ok(link.user_uuid)
    }
}
