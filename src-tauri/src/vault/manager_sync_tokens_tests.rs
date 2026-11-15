use super::*;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("reach-sync-tokens-{}-{}", name, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An identity file from an older version has its token in the clear. The
/// first unlock seals it, and later unlocks still find it.
#[tokio::test]
async fn a_plain_token_is_sealed_on_unlock_and_still_found() {
    let dir = tmp_dir("migrate");
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("token-pass").await.unwrap();

    // Write the token the way older versions did. No URL, so nothing tries
    // to reach a server.
    let path = dir.join("vault_identity.json");
    let mut stored: StoredIdentity = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    stored.personal_sync_token = Some("eyJ.secret-token".into());
    stored.sealed_tokens = None;
    std::fs::write(&path, serde_json::to_string(&stored).unwrap()).unwrap();

    let mut mgr = VaultManager::new(dir.clone());
    assert!(mgr.unlock("token-pass").await.unwrap());
    assert_eq!(mgr.get_personal_sync_config().1.as_deref(), Some("eyJ.secret-token"));

    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(!on_disk.contains("eyJ.secret-token"), "the token is no longer in the file in the clear");
    let stored: StoredIdentity = serde_json::from_str(&on_disk).unwrap();
    assert!(stored.personal_sync_token.is_none());
    assert!(stored.sealed_tokens.is_some());

    let mut mgr = VaultManager::new(dir.clone());
    assert!(mgr.unlock("token-pass").await.unwrap());
    assert_eq!(mgr.get_personal_sync_config().1.as_deref(), Some("eyJ.secret-token"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// A file without any token gets no sealed value either.
#[tokio::test]
async fn no_tokens_means_nothing_sealed() {
    let dir = tmp_dir("none");
    let mut mgr = VaultManager::new(dir.clone());
    mgr.init_identity("token-pass").await.unwrap();
    let stored: StoredIdentity =
        serde_json::from_str(&std::fs::read_to_string(dir.join("vault_identity.json")).unwrap()).unwrap();
    assert!(stored.sealed_tokens.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
