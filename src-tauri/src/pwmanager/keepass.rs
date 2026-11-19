//! KeePass databases (`.kdbx`, as written by KeePass, KeePassXC, KeePassDX,
//! Strongbox and the rest), read in process with the `keepass` crate. No
//! CLI, no other program, so it works wherever Reach does.
//!
//! The database is a file on this computer: its path, and that of a key
//! file if it has one, are this device's own settings and never synced
//! (see [`super::config::DeviceSettings`]). A path on a network share is
//! refused before it is touched, as a CLI's is.
//!
//! The **master password** is asked for in the window when a call needs it,
//! never stored, and kept encrypted in memory for as long as the user said
//! (see [`super::unlock`]). Every call reads and decrypts the file anew, so
//! a password changed in KeePass is the one used at the next connect, and
//! nothing decrypted outlives the call: the file's bytes are wiped, and the
//! opened database is dropped (its protected fields are wiped by the
//! crate) as soon as the listing or the one password has been taken out.
//! Opening is the slow part by design (the key derivation), so it runs on a
//! blocking thread.
//!
//! Reach only reads. It never writes the database, so it cannot conflict
//! with KeePass having it open.

use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};

use ::keepass::db::{Entry, GroupRef};
use ::keepass::error::{DatabaseKeyError, DatabaseOpenError};
use ::keepass::{Database, DatabaseKey};
use zeroize::Zeroizing;

use super::config::KeepassConfig;
use super::unlock::Passphrase;
use super::{sanitize_display, EntrySummary, Failure, PasswordManager, ProviderId, ProviderStatus, SecretKind};

pub(super) const NAME: &str = "KeePass";

/// The largest database file read. Real ones are kilobytes to a few
/// megabytes; attachments can make them bigger.
pub const MAX_DATABASE_BYTES: u64 = 256 * 1024 * 1024;
/// A key file is 32 bytes, a short XML file, or any file the user chose;
/// more than this is not read.
pub const MAX_KEY_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// The longest error text shown.
const MAX_ERROR_CHARS: usize = 300;

/// A stored entry: the entry's UUID.
pub fn validate_entry_id(id: &str) -> Result<uuid::Uuid, String> {
    uuid::Uuid::parse_str(id.trim())
        .map_err(|_| format!("{NAME}: '{}' is not a KeePass entry id", sanitize_display(id.trim(), 60)))
}

/// A database or key file path as it may be used: absolute and on this
/// computer. Checked before anything opens it.
pub fn check_path(raw: &str, what: &str) -> Result<PathBuf, String> {
    let s = raw.trim();
    if super::cli::is_network_path(s) {
        return Err(format!("{NAME}: {what} must be on this computer, not on a network share"));
    }
    let p = Path::new(s);
    if !p.is_absolute() {
        return Err(format!("{NAME}: {what} must be given with its full path"));
    }
    Ok(p.to_path_buf())
}

/// Read a whole file, at most `max` bytes, into a buffer that is wiped.
fn read_capped(path: &Path, max: u64, what: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    let failed = |e: std::io::Error| format!("{NAME}: {what} at {} cannot be read: {}", path.display(), e.kind());
    let file = std::fs::File::open(path).map_err(failed)?;
    let meta = file.metadata().map_err(failed)?;
    if !meta.is_file() {
        return Err(format!("{NAME}: {what} at {} is not a file", path.display()));
    }
    if meta.len() > max {
        return Err(format!("{NAME}: {what} at {} is larger than Reacher CM reads", path.display()));
    }
    // Sized up front, so the buffer is never reallocated (which would leave
    // an unwiped copy behind); one byte more, to notice a file that grew.
    let mut bytes = Zeroizing::new(Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0) + 1));
    file.take(max + 1).read_to_end(&mut bytes).map_err(failed)?;
    if bytes.len() as u64 > max {
        return Err(format!("{NAME}: {what} at {} is larger than Reacher CM reads", path.display()));
    }
    Ok(bytes)
}

/// Open the database at `database` with `password` and, if given, the key
/// file. Blocking: the key derivation takes a moment on purpose.
pub fn open(database: &Path, key_file: Option<&Path>, password: &str) -> Result<Database, Failure> {
    let bytes = read_capped(database, MAX_DATABASE_BYTES, "the database")?;
    let mut key = DatabaseKey::new().with_password(password);
    if let Some(path) = key_file {
        let key_bytes = read_capped(path, MAX_KEY_FILE_BYTES, "the key file")?;
        key = key
            .with_keyfile(&mut key_bytes.as_slice())
            .map_err(|e| format!("{NAME}: the key file at {} cannot be read: {}", path.display(), e.kind()))?;
    }
    Database::parse(&bytes, key).map_err(|e| match e {
        DatabaseOpenError::Key(DatabaseKeyError::IncorrectKey) => Failure::WrongPassphrase(format!(
            "{NAME}: the master password{} was not accepted.",
            if key_file.is_some() { " or key file" } else { "" }
        )),
        DatabaseOpenError::UnsupportedVersion | DatabaseOpenError::VersionParse(_) => {
            Failure::Other(format!("{NAME}: {} is not a KeePass database Reacher CM can read.", database.display()))
        }
        other => Failure::Other(format!(
            "{NAME}: the database could not be opened: {}",
            super::cli::truncate_chars(&sanitize_display(&other.to_string(), MAX_ERROR_CHARS), MAX_ERROR_CHARS)
        )),
    })
}

/// Call `each` with every entry outside the recycle bin and the path of
/// the group it is in (`Servers / Production`).
fn walk(group: &GroupRef<'_>, path: &str, bin: Option<uuid::Uuid>, each: &mut dyn FnMut(&Entry, &str)) {
    for entry in group.entries() {
        each(&entry, path);
    }
    for child in group.groups() {
        if Some(child.id().uuid()) == bin {
            continue;
        }
        let child_path = if path.is_empty() { child.name.clone() } else { format!("{path} / {}", child.name) };
        walk(&child, &child_path, bin, each);
    }
}

fn clean_opt(s: Option<&str>, max: usize) -> Option<String> {
    s.map(|v| sanitize_display(v.trim(), max)).filter(|v| !v.is_empty())
}

/// Every entry that has a password, outside the recycle bin: metadata
/// only. An entry without a title is listed by its user name or address.
pub fn list_entries(db: &Database) -> Vec<EntrySummary> {
    let mut out = Vec::new();
    walk(&db.root(), "", db.meta.recyclebin_uuid, &mut |entry, path| {
        if entry.get_password().is_none_or(str::is_empty) {
            return;
        }
        let username = clean_opt(entry.get_username(), 256);
        let uri = clean_opt(entry.get_url(), 1024);
        let name = clean_opt(entry.get_title(), 256).or_else(|| username.clone()).or_else(|| uri.clone()).unwrap_or_default();
        out.push(EntrySummary {
            id: entry.id().uuid().hyphenated().to_string(),
            name,
            username,
            uri,
            folder_id: clean_opt(Some(path), 512),
        });
    });
    out.sort_by_cached_key(|e| (e.name.to_lowercase(), e.id.clone()));
    out
}

/// The password of the entry `id`, if it is there, outside the recycle
/// bin, and has one.
pub fn entry_password(db: &Database, id: uuid::Uuid) -> Result<Zeroizing<String>, String> {
    let mut found: Option<Zeroizing<String>> = None;
    let mut seen = false;
    walk(&db.root(), "", db.meta.recyclebin_uuid, &mut |entry, _| {
        if entry.id().uuid() == id {
            seen = true;
            found = entry.get_password().filter(|p| !p.is_empty()).map(|p| Zeroizing::new(p.to_owned()));
        }
    });
    match (seen, found) {
        (_, Some(password)) => Ok(password),
        (true, None) => Err(format!("{NAME}: the entry has no password")),
        (false, None) => {
            Err(format!("{NAME}: entry not found. It may have been deleted, or it is in another database."))
        }
    }
}

/// A KeePass database, with the configuration for one call.
pub(crate) struct Keepass {
    cfg: KeepassConfig,
    master_enabled: bool,
}

impl Keepass {
    pub(crate) fn new(cfg: KeepassConfig, master_enabled: bool) -> Self {
        Self { cfg, master_enabled }
    }

    fn database_path(&self) -> Option<&str> {
        self.cfg.database_path.as_deref().map(str::trim).filter(|s| !s.is_empty())
    }

    fn key_file_path(&self) -> Option<&str> {
        self.cfg.key_file_path.as_deref().map(str::trim).filter(|s| !s.is_empty())
    }

    /// The checked paths of the database and its key file.
    fn paths(&self) -> Result<(PathBuf, Option<PathBuf>), String> {
        if !self.cfg.enabled {
            return Err(format!("{NAME} is turned off. Turn it on in Settings → Password managers."));
        }
        let database = self
            .database_path()
            .ok_or_else(|| format!("{NAME}: no database is chosen. Choose it in Settings → Password managers."))?;
        let database = check_path(database, "the database")?;
        let key_file = self.key_file_path().map(|k| check_path(k, "the key file")).transpose()?;
        Ok((database, key_file))
    }

    /// Open the database on a blocking thread and take `take` out of it.
    /// The opened database does not leave that thread.
    async fn with_database<T: Send + 'static>(
        &self,
        passphrase: Option<&Passphrase>,
        take: impl FnOnce(&Database) -> Result<T, String> + Send + 'static,
    ) -> Result<T, Failure> {
        let (database, key_file) = self.paths()?;
        let password =
            passphrase.map(Passphrase::reveal).ok_or_else(|| format!("{NAME}: the master password was not entered."))?;
        tokio::task::spawn_blocking(move || {
            let db = open(&database, key_file.as_deref(), &password)?;
            Ok(take(&db)?)
        })
        .await
        .map_err(|_| Failure::Other(format!("{NAME}: opening the database failed unexpectedly.")))?
    }
}

impl PasswordManager for Keepass {
    async fn status(&self) -> ProviderStatus {
        let mut status = ProviderStatus {
            id: ProviderId::Keepass.as_str().into(),
            name: NAME.into(),
            enabled: self.master_enabled && self.cfg.enabled,
            available: false,
            configured: self.cfg.enabled && self.database_path().is_some(),
            version: None,
            detail: None,
        };
        match self.database_path().map(|p| check_path(p, "the database")) {
            None => status.detail = Some("No database chosen".into()),
            Some(Err(e)) => status.detail = Some(e),
            Some(Ok(path)) if path.is_file() => {
                status.available = true;
                status.detail = Some(path.display().to_string());
            }
            Some(Ok(path)) => status.detail = Some(format!("{NAME}: no database at {}", path.display())),
        }
        status
    }

    /// The master password is always typed, never stored.
    fn passphrase_prompt(&self) -> Option<u32> {
        Some(self.cfg.remember_secs())
    }

    fn secret_kind(&self) -> SecretKind {
        SecretKind::MasterPassword
    }

    fn server_host(&self) -> Option<String> {
        None
    }

    fn hash_account(&self, mut h: &mut dyn Hasher) {
        self.database_path().hash(&mut h);
        self.key_file_path().hash(&mut h);
    }

    /// The password only, for now.
    fn check_entry(&self, entry_id: &str, field: Option<&str>) -> Result<String, String> {
        if let Some(f) = field {
            return Err(super::unsupported_field(ProviderId::Keepass, f));
        }
        validate_entry_id(entry_id).map(|u| u.hyphenated().to_string())
    }

    async fn list(&self, passphrase: Option<&Passphrase>) -> Result<Vec<EntrySummary>, Failure> {
        self.with_database(passphrase, |db| Ok(list_entries(db))).await
    }

    async fn resolve(&self, entry_id: &str, passphrase: Option<&Passphrase>) -> Result<Zeroizing<String>, Failure> {
        let id = validate_entry_id(entry_id)?;
        self.with_database(passphrase, move |db| entry_password(db, id)).await
    }
}

#[cfg(test)]
#[path = "keepass_tests.rs"]
mod tests;
