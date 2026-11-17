use std::path::PathBuf;

use ::keepass::db::GroupId;

use super::*;

const PASSWORD: &str = "correct horse";

struct Fixture {
    dir: PathBuf,
    database: PathBuf,
    key_file: PathBuf,
    prod: uuid::Uuid,
    binned: uuid::Uuid,
    empty: uuid::Uuid,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn add_entry(db: &mut Database, group: Option<GroupId>, title: &str, user: &str, url: &str, password: &str) -> uuid::Uuid {
    let mut root = db.root_mut();
    let id = match group {
        Some(id) => root.group_mut(id).unwrap().add_entry().id(),
        None => root.add_entry().id(),
    };
    let mut e = db.entry_mut(id).unwrap();
    e.set_unprotected("Title", title);
    e.set_unprotected("UserName", user);
    e.set_unprotected("URL", url);
    e.set_protected("Password", password);
    id.uuid()
}

/// A database with an entry in a group, one in the recycle bin, one
/// without a password and a note; saved with a password, or with a
/// password and a key file.
fn fixture(with_key_file: bool) -> Fixture {
    let dir = std::env::temp_dir().join(format!("reach-kdbx-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut db = Database::new();
    let servers = {
        let mut root = db.root_mut();
        let mut g = root.add_group();
        g.name = "Servers".into();
        g.id()
    };
    let bin = {
        let mut root = db.root_mut();
        let mut g = root.add_group();
        g.name = "Recycle Bin".into();
        g.id()
    };
    db.meta.recyclebin_uuid = Some(bin.uuid());
    let prod = add_entry(&mut db, Some(servers), "prod db", "admin", "ssh://db.example.com", "hunter2");
    add_entry(&mut db, None, "", "root", "", "toor");
    let binned = add_entry(&mut db, Some(bin), "old", "x", "", "binned-secret");
    let empty = add_entry(&mut db, None, "no password", "x", "", "");

    let key_file = dir.join("db.key");
    let mut key = DatabaseKey::new().with_password(PASSWORD);
    if with_key_file {
        std::fs::write(&key_file, [7u8; 32]).unwrap();
        key = key.with_keyfile(&mut std::fs::File::open(&key_file).unwrap()).unwrap();
    }
    let database = dir.join("test.kdbx");
    db.save(&mut std::fs::File::create(&database).unwrap(), key).unwrap();
    Fixture { dir, database, key_file, prod, binned, empty }
}

fn provider(f: &Fixture, with_key_file: bool) -> Keepass {
    Keepass::new(
        KeepassConfig {
            database_path: Some(f.database.to_str().unwrap().into()),
            key_file_path: with_key_file.then(|| f.key_file.to_str().unwrap().into()),
            enabled: true,
            ..Default::default()
        },
        true,
    )
}

fn master(s: &str) -> Passphrase {
    Passphrase::new(s.into()).unwrap()
}

#[tokio::test]
async fn entries_are_listed_without_their_passwords() {
    let f = fixture(false);
    let p = provider(&f, false);
    let list = p.list(Some(&master(PASSWORD))).await.map_err(Failure::into_message).unwrap();
    assert_eq!(list.len(), 2, "the binned entry and the one without a password are left out: {list:?}");
    assert_eq!(list[0].name, "prod db");
    assert_eq!(list[0].id, f.prod.hyphenated().to_string());
    assert_eq!(list[0].username.as_deref(), Some("admin"));
    assert_eq!(list[0].uri.as_deref(), Some("ssh://db.example.com"));
    assert_eq!(list[0].folder_id.as_deref(), Some("Servers"));
    assert_eq!(list[1].name, "root", "an untitled entry goes by its user name");
    let shown = format!("{list:?}");
    for secret in ["hunter2", "toor", "binned-secret"] {
        assert!(!shown.contains(secret), "{secret} is in the listing");
    }
}

#[tokio::test]
async fn a_password_is_read_by_entry_id() {
    let f = fixture(false);
    let p = provider(&f, false);
    let m = master(PASSWORD);
    let read = |id: uuid::Uuid| {
        let (p, m) = (&p, &m);
        async move { p.resolve(&id.to_string(), Some(m)).await.map_err(Failure::into_message) }
    };
    assert_eq!(read(f.prod).await.unwrap().as_str(), "hunter2");
    assert!(read(f.binned).await.unwrap_err().contains("not found"), "the recycle bin is not read");
    assert!(read(f.empty).await.unwrap_err().contains("no password"));
    assert!(read(uuid::Uuid::new_v4()).await.unwrap_err().contains("not found"));
    assert!(p.resolve("--x", Some(&m)).await.is_err());
}

#[tokio::test]
async fn a_wrong_master_password_is_asked_for_again() {
    let f = fixture(false);
    let p = provider(&f, false);
    assert!(matches!(p.list(Some(&master("wrong"))).await, Err(Failure::WrongPassphrase(m)) if m.contains("master password")));
    assert!(matches!(p.list(None).await, Err(Failure::Other(_))));
}

#[tokio::test]
async fn a_key_file_is_part_of_the_key() {
    let f = fixture(true);
    let m = master(PASSWORD);
    assert_eq!(provider(&f, true).list(Some(&m)).await.map_err(Failure::into_message).unwrap().len(), 2);
    assert!(matches!(provider(&f, false).list(Some(&m)).await, Err(Failure::WrongPassphrase(_))));
    std::fs::write(&f.key_file, [8u8; 32]).unwrap();
    assert!(matches!(provider(&f, true).list(Some(&m)).await, Err(Failure::WrongPassphrase(m)) if m.contains("key file")));
}

#[tokio::test]
async fn what_is_not_a_database_is_said_so() {
    let f = fixture(false);
    std::fs::write(&f.database, b"this is not a kdbx file at all").unwrap();
    let e = provider(&f, false).list(Some(&master(PASSWORD))).await.unwrap_err();
    assert!(matches!(&e, Failure::Other(m) if m.contains("not a KeePass database")), "{e:?}");
    std::fs::remove_file(&f.database).unwrap();
    let e = provider(&f, false).list(Some(&master(PASSWORD))).await.unwrap_err();
    assert!(matches!(&e, Failure::Other(m) if m.contains("cannot be read")), "{e:?}");
}

#[test]
fn paths_must_be_absolute_and_on_this_computer() {
    assert!(check_path("relative/db.kdbx", "the database").unwrap_err().contains("full path"));
    assert!(check_path("\\\\server\\share\\db.kdbx", "the database").unwrap_err().contains("network share"));
    assert!(check_path("//server/share/db.kdbx", "the key file").unwrap_err().contains("the key file"));
    let abs = if cfg!(windows) { "C:\\db.kdbx" } else { "/home/u/db.kdbx" };
    assert!(check_path(abs, "the database").is_ok());
}

#[tokio::test]
async fn status_says_whether_the_database_is_there() {
    let f = fixture(false);
    let s = provider(&f, false).status().await;
    assert!(s.available && s.configured && s.enabled);
    let none = Keepass::new(KeepassConfig { enabled: true, ..Default::default() }, true);
    let s = none.status().await;
    assert!(!s.available && !s.configured);
    assert!(matches!(none.list(Some(&master(PASSWORD))).await, Err(Failure::Other(m)) if m.contains("no database is chosen")));
    let off = Keepass::new(KeepassConfig { database_path: Some(f.database.to_str().unwrap().into()), ..Default::default() }, true);
    assert!(matches!(off.list(Some(&master(PASSWORD))).await, Err(Failure::Other(m)) if m.contains("turned off")));
}

#[test]
fn the_account_follows_the_files_and_only_the_password_is_read() {
    use std::hash::{DefaultHasher, Hasher};
    let hash = |db: &str, key: Option<&str>| {
        let mut h = DefaultHasher::new();
        Keepass::new(
            KeepassConfig { database_path: Some(db.into()), key_file_path: key.map(Into::into), ..Default::default() },
            true,
        )
        .hash_account(&mut h);
        h.finish()
    };
    assert_eq!(hash("/a.kdbx", None), hash(" /a.kdbx ", None));
    assert_ne!(hash("/a.kdbx", None), hash("/b.kdbx", None));
    assert_ne!(hash("/a.kdbx", None), hash("/a.kdbx", Some("/a.key")));

    let p = Keepass::new(KeepassConfig::default(), true);
    let id = "3F2C1A9E-5B7D-4E0A-9C1B-2D3E4F5A6B7C";
    assert_eq!(p.check_entry(id, None).unwrap(), id.to_lowercase());
    assert!(p.check_entry(id, Some("otp")).is_err());
    assert_eq!(p.passphrase_prompt(), Some(crate::pwmanager::config::DEFAULT_REMEMBER_SECS));
    assert_eq!(p.secret_kind(), SecretKind::MasterPassword);
}

/// Against a database written by another program:
/// `REACH_KDBX=/path/db.kdbx REACH_KDBX_PASSWORD=… cargo test --lib keepass_live -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn keepass_live() {
    let (Ok(path), Ok(password)) = (std::env::var("REACH_KDBX"), std::env::var("REACH_KDBX_PASSWORD")) else {
        return;
    };
    let p = Keepass::new(
        KeepassConfig {
            database_path: Some(path),
            key_file_path: std::env::var("REACH_KDBX_KEY_FILE").ok(),
            enabled: true,
            ..Default::default()
        },
        true,
    );
    let m = master(&password);
    let list = p.list(Some(&m)).await.map_err(Failure::into_message).unwrap();
    println!("{} entries", list.len());
    if let Some(first) = list.first() {
        let secret = p.resolve(&first.id, Some(&m)).await.map_err(Failure::into_message).unwrap();
        println!("read {} in {:?}: {} characters", first.name, first.folder_id, secret.chars().count());
    }
}
