use super::*;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("reach-device-link-{}-{}", name, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A second device that set up an identity of its own (and so a vault of
/// its own) joins the first device's vault from a link code: it opens the
/// same personal vault, reads what the first device wrote, and its own
/// master password is now the first device's.
#[tokio::test]
async fn a_linked_device_opens_the_vault_the_first_one_syncs() {
    let dir = tmp_dir("join");
    let share = dir.join("share");
    std::fs::create_dir_all(&share).unwrap();
    let url = format!("folder:{}", share.display());
    let put = |text: &str| SecretBox::new(Box::new(text.as_bytes().to_vec()));

    let mut a = VaultManager::new(dir.join("a"));
    a.init_identity("desktop-pass").await.unwrap();
    assert!(a.export_device_link().await.is_err(), "nothing to link before sync is on");
    a.set_personal_sync_config(Some(url.clone()), Some(String::new())).await.unwrap();
    let vault = a.vault_names.get(SETTINGS_VAULT).cloned().unwrap();
    let one = a.create_secret(&vault, "Router", SecretCategory::Session, put("one")).await.unwrap();
    let code = a.export_device_link().await.unwrap();
    assert!(device_link::is_link_code(&code));
    let a = tokio::sync::Mutex::new(a);
    let (_, failed) = sync_vaults(&a, None, false).await;
    assert!(failed.is_empty());

    // The phone made its own identity first, as it did before linking existed.
    let mut b = VaultManager::new(dir.join("b"));
    b.init_identity("phone-pass").await.unwrap();
    let uuid = b.link_device(&format!("  {}\n", code)).await.unwrap();
    assert_eq!(Some(uuid), a.lock().await.user_uuid.clone());
    assert_eq!(b.vault_names.get(SETTINGS_VAULT), Some(&vault), "the same vault, not a new one");
    assert_ne!(b.device_id, a.lock().await.device_id, "another device of the same user");
    let b = tokio::sync::Mutex::new(b);
    let (_, failed) = sync_vaults(&b, None, false).await;
    assert!(failed.is_empty(), "{:?}", failed.iter().map(|(_, e)| e.to_string()).collect::<Vec<_>>());
    assert_eq!(b.lock().await.read_secret(&vault, &one).await.unwrap().expose_secret(), b"one");

    // A change on the phone reaches the desktop.
    let two = b.lock().await.create_secret(&vault, "Switch", SecretCategory::Session, put("two")).await.unwrap();
    sync_vaults(&b, None, false).await;
    sync_vaults(&a, None, false).await;
    assert_eq!(a.lock().await.read_secret(&vault, &two).await.unwrap().expose_secret(), b"two");

    // The token is sealed on disk, and the desktop's password opens the phone.
    let file = std::fs::read_to_string(dir.join("b/vault_identity.json")).unwrap();
    let stored: StoredIdentity = serde_json::from_str(&file).unwrap();
    assert!(stored.personal_sync_token.is_none());
    let mut b = VaultManager::new(dir.join("b"));
    assert!(!b.unlock("phone-pass").await.unwrap_or(false));
    assert!(b.unlock("desktop-pass").await.unwrap());
    assert_eq!(b.vault_names.get(SETTINGS_VAULT), Some(&vault));

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_damaged_code_changes_nothing() {
    let dir = tmp_dir("damaged");
    let mut b = VaultManager::new(dir.clone());
    let uuid = b.init_identity("pass").await.unwrap();
    assert!(b.link_device("reachercm-link1:not-base64!").await.is_err());
    assert!(b.link_device("reachercm-link1:e30").await.is_err());
    assert_eq!(b.user_uuid, Some(uuid));
    let _ = std::fs::remove_dir_all(&dir);
}
