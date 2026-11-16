//! What every manager driven through a command-line tool shares: finding
//! and checking the binary, and the child's starting environment.
//!
//! Each such manager describes its tool once, in a [`CliSpec`], and calls
//! these with it. The rules are the same for all of them, because the
//! binary is handed secrets (a private key, a master password, a session
//! token):
//!
//! - The binary has the tool's own file name ([`CliSpec::file_names`]), so a
//!   script (`.cmd`, `.ps1`, `.sh`) run through an interpreter is refused.
//! - It is on this computer: a network path (`\\server\share\…`) is refused
//!   before it is touched, since opening one sends the user's NTLM hash.
//! - On Unix it and its folder are owned by the user or root and writable
//!   by no one else; on Windows only the user or the system can change them
//!   (`super::winacl`). Whether it was set in Settings or found on PATH.
//! - The child's environment starts empty: only [`CliSpec::inherited_env`]
//!   is copied from Reach's own, and [`CliSpec::removed_env`] (the tool's
//!   own settings variables) is removed by name in case one slips through.
//!
//! How a call is run (a slot, a deadline, capped output) is `super::process`.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::sanitize_display;

/// One command-line tool, as Reach runs it.
#[derive(Debug)]
pub(crate) struct CliSpec {
    /// The manager's name, at the start of every message: `"Passbolt"`.
    pub name: &'static str,
    /// The program looked for on PATH: `"passbolt"`.
    pub program: &'static str,
    /// The file names the binary may have, compared without regard to case.
    pub file_names: &'static [&'static str],
    /// How to install the tool, for "not found".
    pub install_hint: &'static str,
    /// What the child inherits from Reach's environment, and nothing more.
    /// Matched exactly on Unix and without regard to case on Windows.
    pub inherited_env: &'static [&'static str],
    /// Removed from the child's environment by name after the allowlist,
    /// in case one is ever added to it by mistake.
    pub removed_env: &'static [&'static str],
}

/// Whether `name` is in the spec's [`CliSpec::inherited_env`].
pub(crate) fn is_inherited_env(spec: &CliSpec, name: &OsStr) -> bool {
    let Some(name) = name.to_str() else { return false };
    if cfg!(windows) {
        spec.inherited_env.iter().any(|k| k.eq_ignore_ascii_case(name))
    } else {
        spec.inherited_env.contains(&name)
    }
}

/// A path that names a network location (`\\server\share\…`, `//server/…`,
/// `\\?\UNC\…`). Opening one makes Windows sign in to that server with the
/// user's NTLM credentials, so such paths are refused before anything
/// touches them.
pub fn is_network_path(path: &str) -> bool {
    let p = path.trim_start();
    let mut chars = p.chars();
    matches!((chars.next(), chars.next()), (Some('\\' | '/'), Some('\\' | '/')))
}

/// The checks on a CLI path that need no file system: absolute, on this
/// computer, and named as the tool is.
pub(crate) fn check_cli_path_shape(spec: &CliSpec, path: &Path) -> Result<(), String> {
    let name = spec.name;
    if is_network_path(&path.to_string_lossy()) {
        return Err(format!("{name}: the CLI must be on this computer, not on a network share"));
    }
    if !path.is_absolute() {
        return Err(format!("{name}: the CLI path must be absolute"));
    }
    let file = path.file_name().and_then(OsStr::to_str).unwrap_or("");
    if !spec.file_names.iter().any(|n| n.eq_ignore_ascii_case(file)) {
        return Err(format!(
            "{name}: the CLI must be the {} program itself (a file named {}), not '{}'",
            spec.program,
            spec.file_names.join(" or "),
            sanitize_display(file, 80)
        ));
    }
    Ok(())
}

/// The checks that look at the file: it exists and is a file, and neither
/// it nor its folder can be changed by anyone but the user or the system
/// (root on Unix; LocalSystem, Administrators or TrustedInstaller on
/// Windows). Returns the path to run: on Unix the real file behind any
/// symlink, so what runs is what was checked.
pub(crate) fn check_cli_file(spec: &CliSpec, path: &Path) -> Result<PathBuf, String> {
    let name = spec.name;
    check_cli_path_shape(spec, path)?;
    if !path.is_file() {
        return Err(format!("{name}: no CLI at {}. {}", path.display(), spec.install_hint));
    }
    #[cfg(unix)]
    {
        let real = std::fs::canonicalize(path)
            .map_err(|e| format!("{name}: the CLI at {} cannot be read: {}", path.display(), e.kind()))?;
        check_unix_owner(name, &real, "the CLI")?;
        for dir in [real.parent(), path.parent()].into_iter().flatten() {
            check_unix_owner(name, dir, "the CLI's folder")?;
        }
        Ok(real)
    }
    #[cfg(windows)]
    {
        // A link (winget's Links folder) is checked with the file it leads
        // to; the link itself is what runs, and its folder is checked too.
        let real = std::fs::canonicalize(path)
            .map_err(|e| format!("{name}: the CLI at {} cannot be read: {}", path.display(), e.kind()))?;
        super::winacl::check_not_replaceable(name, &real, "the CLI")?;
        for dir in [real.parent(), path.parent()].into_iter().flatten() {
            super::winacl::check_not_replaceable(name, dir, "the CLI's folder")?;
        }
        Ok(path.to_path_buf())
    }
    #[cfg(not(any(unix, windows)))]
    Ok(path.to_path_buf())
}

/// Owned by the user running Reach or by root, and not writable by its group
/// or by others: otherwise someone else could swap in their own program and
/// be handed the secrets it gets.
#[cfg(unix)]
pub(crate) fn check_unix_owner(name: &str, path: &Path, what: &str) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("{name}: {what} at {} cannot be read: {}", path.display(), e.kind()))?;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let me = unsafe { libc::geteuid() };
    if meta.uid() != me && meta.uid() != 0 {
        return Err(format!("{name}: {what} at {} belongs to another user", path.display()));
    }
    if !writable_only_by_trusted(meta.mode(), || group_holds_only(meta.gid(), me)) {
        return Err(format!(
            "{name}: {what} at {} can be changed by other users (group or world writable)",
            path.display()
        ));
    }
    Ok(())
}

/// Nobody but the owner, root or members of a group that holds only the
/// user and root can write. Never world writable. Group writable is fine
/// when `group_is_mine` says so: Homebrew's `/usr/local/bin` on an Intel
/// Mac (`user:admin 775`, admin = root and the user) and `~/.local/bin`
/// with a user-private group on Linux (`user:user 775`).
#[cfg(unix)]
pub(crate) fn writable_only_by_trusted(mode: u32, group_is_mine: impl FnOnce() -> bool) -> bool {
    if mode & 0o002 != 0 {
        return false;
    }
    mode & 0o020 == 0 || group_is_mine()
}

/// Whether every member of group `gid` is root or `me`: those listed in the
/// group, and those whose primary group it is (which the group's own list
/// leaves out). Anything that cannot be read counts as "no".
#[cfg(unix)]
pub(crate) fn group_holds_only(gid: u32, me: u32) -> bool {
    match group_members(gid) {
        Some(uids) => uids.iter().all(|&u| u == 0 || u == me),
        None => false,
    }
}

/// The uids in group `gid`, or `None` if the system cannot say.
#[cfg(unix)]
fn group_members(gid: u32) -> Option<Vec<u32>> {
    use std::ffi::CStr;
    use std::sync::Mutex;
    // getgrgid, getpwnam and getpwent share static buffers: one caller at a
    // time inside Reach.
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut uids = Vec::new();
    // SAFETY: the returned pointers are read before the next call to the
    // same family, under the lock; null is checked.
    unsafe {
        let gr = libc::getgrgid(gid as libc::gid_t);
        if gr.is_null() {
            return None;
        }
        let mut names = Vec::new();
        let mut m = (*gr).gr_mem;
        while !m.is_null() && !(*m).is_null() {
            names.push(CStr::from_ptr(*m).to_owned());
            m = m.add(1);
        }
        for name in names {
            let pw = libc::getpwnam(name.as_ptr());
            if pw.is_null() {
                // A member we cannot resolve: not provably one of us.
                return None;
            }
            uids.push((*pw).pw_uid as u32);
        }
        libc::setpwent();
        loop {
            let pw = libc::getpwent();
            if pw.is_null() {
                break;
            }
            if (*pw).pw_gid as u32 == gid {
                uids.push((*pw).pw_uid as u32);
            }
        }
        libc::endpwent();
    }
    Some(uids)
}

/// The CLI binary: the configured path, or the program found on PATH.
/// Always an absolute path to an existing file that passed
/// [`check_cli_file`].
pub(crate) fn resolve_binary(spec: &CliSpec, cli_path: Option<&str>) -> Result<PathBuf, String> {
    match cli_path.map(str::trim).filter(|s| !s.is_empty()) {
        Some(p) => check_cli_file(spec, Path::new(p)),
        None => {
            let found = which::which(spec.program).map_err(|_| {
                format!("{}: the {} CLI was not found. {}", spec.name, spec.program, spec.install_hint)
            })?;
            let found = if found.is_absolute() { found } else { std::path::absolute(&found).unwrap_or(found) };
            // On Windows PATHEXT also finds .cmd or .bat files, and a PATH
            // entry may be a network share: the same rules as a path set in
            // Settings.
            check_cli_file(spec, &found)
        }
    }
}

/// Empty the child's environment, then copy back what the spec allows from
/// `parent`, then remove the tool's own settings variables by name.
pub(crate) fn set_base_env(
    spec: &CliSpec,
    cmd: &mut tokio::process::Command,
    parent: impl IntoIterator<Item = (OsString, OsString)>,
) {
    cmd.env_clear();
    for (k, v) in parent {
        if is_inherited_env(spec, &k) {
            cmd.env(k, v);
        }
    }
    for key in spec.removed_env {
        cmd.env_remove(key);
    }
}

/// Drop ANSI escape sequences and every control character but newlines.
pub fn strip_control(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI: ESC [ params… final byte (0x40–0x7e). Other escapes: ESC
            // and the next character.
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&n) {
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else if c == '\n' || !c.is_control() {
            out.push(c);
        }
    }
    out
}

/// `s`, cut to `max` characters with an ellipsis if it was longer.
pub(crate) fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
