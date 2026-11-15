//! Files that hold vault data are for the user who owns them.
//!
//! On Linux and macOS a file is created readable by everyone unless the
//! process says otherwise (umask 022). The identity file, the vault databases
//! and their caches are encrypted, but they still name the user's vaults and
//! sync targets, so they are written owner-only (0600) in a directory only the
//! owner can enter (0700). Windows keeps a user's app data under their own
//! profile, whose ACL already does this, so nothing changes there.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Write `data` to `path` as an owner-only file. It is written beside the
/// target and renamed over it, so a crash part way leaves the old file or the
/// new one, never half of each.
pub fn write(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = tmp_path(path);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(&tmp)?;
    // A temp file left by an earlier crash keeps the mode it was made with.
    restrict_file(&tmp)?;
    file.write_all(data)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, path)
}

/// Async form of [`write`] for callers on the runtime.
pub async fn write_async(path: &Path, data: Vec<u8>) -> io::Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || write(&path, &data))
        .await
        .map_err(io::Error::other)?
}

/// Make an existing file owner-only. A missing file is not an error.
pub fn restrict_file(path: &Path) -> io::Result<()> {
    set_mode(path, 0o600)
}

/// Create `dir` if needed and make it owner-only.
pub fn private_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    set_mode(dir, 0o700)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(meta) if meta.permissions().mode() & 0o777 == mode => Ok(()),
        Ok(_) => std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("reach-private-fs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_and_replaces() {
        let dir = tempdir();
        let path = dir.join("a.json");
        write(&path, b"one").unwrap();
        write(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        assert!(!dir.join("a.json.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn files_and_dirs_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir();
        let sub = dir.join("vaults");
        std::fs::create_dir(&sub).unwrap();
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o755)).unwrap();
        private_dir(&sub).unwrap();
        assert_eq!(std::fs::metadata(&sub).unwrap().permissions().mode() & 0o777, 0o700);

        let path = sub.join("id.json");
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        write(&path, b"new").unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
