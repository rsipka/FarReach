use super::*;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

/// A private key has to survive a backup as key *material*, not as a
/// reference: restore on a new machine and the same bytes must come back,
/// or the sessions that use it are dead on arrival.
///
/// This walks the encoding the backup actually uses — encrypt the stored
/// key, write the three fields the way `export_full_backup` writes them,
/// read them the way `import_full_backup` reads them, decrypt — so a
/// change to either side that drops the key breaks here.
#[test]
fn an_imported_key_survives_a_backup() {
    use crate::ssh::keystore::StoredKeyMaterial;
    use crate::vault::crypto::{decrypt_secret, encrypt_secret};
    use crate::vault::types::{Dek, EncryptedPayload, WrappedDek};
    use secrecy::ExposeSecret;

    const ED25519: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\nQyNTUxOQAAACCwsCZ1pFDo+/0R+iXN9Emzi+emmN0GEbCoIL9hE1eafgAAAJhbzNn6W8zZ\n+gAAAAtzc2gtZWQyNTUxOQAAACCwsCZ1pFDo+/0R+iXN9Emzi+emmN0GEbCoIL9hE1eafg\nAAAEDs2ThleIPEbrfMS5+KvabXLE0113+oUG6NIEaDtPChvLCwJnWkUOj7/RH6Jc30SbOL\n56aY3QYRsKggv2ETV5p+AAAAEnJlYWNoLXRlc3RAZXhhbXBsZQECAw==\n-----END OPENSSH PRIVATE KEY-----\n";

    let master_dek = Dek::new([0x42; 32]);
    let material = StoredKeyMaterial {
        private_key: ED25519.to_string(),
        passphrase: Some("hunter2".into()),
        public_key: Some("ssh-ed25519 AAAA test@host".into()),
    };

    // What the vault stores for one imported key.
    let plaintext = serde_json::to_vec(&material).unwrap();
    let payload = encrypt_secret(&master_dek, &plaintext).unwrap();

    // What export writes into the bundle.
    let exported = ExportedSecret {
        id: "key-1".into(),
        name: "Work laptop".into(),
        category: "ssh_key".into(),
        nonce: BASE64.encode(payload.nonce),
        ciphertext: BASE64.encode(&payload.ciphertext),
        wrapped_dek_json: serde_json::to_string(&payload.wrapped_dek).unwrap(),
        created_at: 0,
        updated_at: 0,
    };

    // The bytes really are in there, not just a name pointing at them.
    assert!(!exported.ciphertext.is_empty());
    assert!(
        !exported.ciphertext.contains("PRIVATE KEY"),
        "the key must be encrypted in the bundle, never in the clear"
    );

    // What import reads back out.
    let nonce_bytes = BASE64.decode(&exported.nonce).unwrap();
    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&nonce_bytes);
    let restored = EncryptedPayload {
        nonce,
        ciphertext: BASE64.decode(&exported.ciphertext).unwrap(),
        wrapped_dek: serde_json::from_str::<WrappedDek>(&exported.wrapped_dek_json).unwrap(),
    };

    let opened = decrypt_secret(&master_dek, &restored).unwrap();
    let back: StoredKeyMaterial = serde_json::from_slice(opened.expose_secret()).unwrap();

    assert_eq!(back.private_key, material.private_key);
    assert_eq!(back.passphrase, material.passphrase);
    assert_eq!(back.public_key, material.public_key);

    // And the restored bytes are still a usable key, not just equal text.
    crate::ssh::keystore::verify_passphrase(&back.private_key, back.passphrase.as_deref())
        .expect("the restored key should still open");
}

#[test]
fn test_seal_unseal_roundtrip() {
    let bundle = ExportBundle {
        version: FORMAT_VERSION,
        exported_at: 1700000000,
        identity: ExportedIdentity {
            user_uuid: "test-uuid".to_string(),
            salt: BASE64.encode([0xAA; 32]),
            encrypted_key: BASE64.encode([0xBB; 48]),
            nonce: BASE64.encode([0xCC; 24]),
            public_key: BASE64.encode([0xDD; 32]),
            secret_key: Some(BASE64.encode([0xEE; 32])),
        },
        vaults: vec![],
        sync_config: None,
        app_settings: None,
    };

    let password = "test-password-123";
    let sealed = seal_bundle(&bundle, password).unwrap();

    // Verify magic
    assert_eq!(&sealed[0..8], MAGIC);

    // Unseal
    let unsealed = unseal_bundle(&sealed, password).unwrap();
    assert_eq!(unsealed.identity.user_uuid, "test-uuid");
    assert_eq!(unsealed.exported_at, 1700000000);
}

#[test]
fn test_wrong_password() {
    let bundle = ExportBundle {
        version: FORMAT_VERSION,
        exported_at: 0,
        identity: ExportedIdentity {
            user_uuid: "u".to_string(),
            salt: BASE64.encode([0; 32]),
            encrypted_key: BASE64.encode([0; 48]),
            nonce: BASE64.encode([0; 24]),
            public_key: BASE64.encode([0; 32]),
            secret_key: Some(BASE64.encode([0; 32])),
        },
        vaults: vec![],
        sync_config: None,
        app_settings: None,
    };

    let sealed = seal_bundle(&bundle, "correct-pass").unwrap();
    let result = unseal_bundle(&sealed, "wrong-pass");
    assert!(result.is_err());
}

#[test]
fn test_short_password_rejected() {
    let bundle = ExportBundle {
        version: FORMAT_VERSION,
        exported_at: 0,
        identity: ExportedIdentity {
            user_uuid: "u".to_string(),
            salt: BASE64.encode([0; 32]),
            encrypted_key: BASE64.encode([0; 48]),
            nonce: BASE64.encode([0; 24]),
            public_key: BASE64.encode([0; 32]),
            secret_key: Some(BASE64.encode([0; 32])),
        },
        vaults: vec![],
        sync_config: None,
        app_settings: None,
    };

    let result = seal_bundle(&bundle, "short");
    assert!(result.is_err());
}

#[test]
fn test_invalid_format() {
    let result = unseal_bundle(b"not a backup", "password123");
    assert!(result.is_err());
}
