use super::*;
use crate::vault::types::{MemberRole, VaultType};
use crate::vault::VaultManager;

fn device(name: &str) -> VaultManager {
    let dir = std::env::temp_dir().join(format!("reach-folder-sync-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    VaultManager::new(dir)
}

fn put(folder: &Folder) -> SecretBox<Vec<u8>> {
    SecretBox::new(Box::new(serde_json::to_vec(folder).unwrap()))
}

fn names(folders: &[Folder]) -> Vec<String> {
    let mut names: Vec<String> = folders.iter().map(|f| f.name.clone()).collect();
    names.sort();
    names
}

#[tokio::test]
#[ignore]
async fn a_shared_vaults_folders_reach_the_other_member() {
    let Ok(url) = std::env::var("REACH_TEST_SQLD") else { return };
    let server = libsql::Builder::new_remote(url.clone(), String::new())
        .connector(crate::vault::turso_tls::TursoConnector::new().unwrap())
        .build()
        .await
        .unwrap();
    server
        .connect()
        .unwrap()
        .execute_batch("DROP TABLE IF EXISTS secrets; DROP TABLE IF EXISTS vault_members; DROP TABLE IF EXISTS vault_header;")
        .await
        .unwrap();

    let mut a = device("a");
    a.init_identity("folder-sync-a").await.unwrap();
    let team = a.create_vault("Team", VaultType::Private, Some(&url), Some("")).await.unwrap().id;

    // As Reach before 0.8.2 left them: both folders on the device, one
    // naming the shared vault.
    let local = ensure_folders_vault(&mut a).await.unwrap();
    let old = Folder { id: "old".into(), name: "Servers".into(), parent_id: None, vault_id: Some(team.clone()), credential_id: None };
    let mine = Folder { id: "mine".into(), name: "Home".into(), parent_id: None, vault_id: None, credential_id: None };
    for f in [&old, &mine] {
        a.create_secret_with_id(&local, &f.id, &f.name, SecretCategory::Folder, put(f)).await.unwrap();
    }

    move_folders_to_their_vaults(&a).await;
    assert!(a.secret_exists(&team, "old").await, "the shared folder is in its vault");
    assert!(!a.secret_exists(&local, "old").await, "and no longer on the device");
    assert!(a.secret_exists(&local, "mine").await, "a folder of the device's own sessions stays");
    move_folders_to_their_vaults(&a).await;
    assert_eq!(names(&read_folders(&a).await.unwrap()), ["Home", "Servers"], "a second run changes nothing");

    // A new folder in the shared vault, nested under the moved one.
    let web = create_folder(&mut a, "Web".into(), Some("old".into()), Some(team.clone())).await.unwrap();
    assert!(a.secret_exists(&team, &web.id).await);
    assert!(create_folder(&mut a, "Mixed".into(), Some("mine".into()), Some(team.clone())).await.is_err());

    // Device B joins; the vault has its own id there.
    let mut b = device("b");
    b.init_identity("folder-sync-b").await.unwrap();
    let b_key: [u8; 32] = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b.get_public_key().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let invite = a.invite_member(&team, &b_key, &b.get_user_uuid().unwrap(), MemberRole::Member).await.unwrap();
    let b_team = b.accept_invite(&invite.sync_url, &invite.token).await.unwrap().id;
    assert_ne!(b_team, team);

    let seen = read_folders(&b).await.unwrap();
    assert_eq!(names(&seen), ["Servers", "Web"], "B sees the shared folders, not A's own");
    assert!(seen.iter().all(|f| f.vault_id.as_deref() == Some(b_team.as_str())), "under B's id for the vault");
    assert_eq!(seen.iter().find(|f| f.name == "Web").unwrap().parent_id.as_deref(), Some("old"));

    // B moves Web to the top and deletes Servers; A sees both.
    set_folder_parent(&b, web.id.clone(), None).await.unwrap();
    delete_folder(&b, "old").await.unwrap();
    a.sync_vault(&team).await.unwrap();
    let after = read_folders(&a).await.unwrap();
    assert_eq!(names(&after), ["Home", "Web"]);
    assert_eq!(after.iter().find(|f| f.name == "Web").unwrap().parent_id, None);

    // A copy left on the device by a move cut short goes with the folder.
    a.create_secret_with_id(&local, &web.id, "Web", SecretCategory::Folder, put(&web)).await.unwrap();
    delete_folder(&a, &web.id).await.unwrap();
    move_folders_to_their_vaults(&a).await;
    assert_eq!(names(&read_folders(&a).await.unwrap()), ["Home"]);
}
