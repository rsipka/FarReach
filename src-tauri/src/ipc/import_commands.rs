//! IPC commands for importing sessions from other connection managers.
//!
//! The readers themselves are in TypeScript (src/lib/sessions/import); this
//! side only fetches what the webview cannot: where those apps keep their
//! settings on this computer (files under the user's profile, and on
//! Windows the registry, through `reg export`), and YAML turned into JSON
//! for Tabby's config.

use std::path::{Path, PathBuf};

/// Larger than any real settings file; stops a wrong path from filling memory.
const MAX_BYTES: u64 = 50 * 1024 * 1024;

/// A YAML document (Tabby's config.yaml) as JSON.
#[tauri::command]
pub fn import_yaml_to_json(text: String) -> Result<serde_json::Value, String> {
    serde_saphyr::from_str::<serde_json::Value>(&text).map_err(|e| format!("Not valid YAML: {e}"))
}

/// What `source` keeps on this computer, as raw file contents (one or more
/// pieces, each decoded by the webview), or None when it is not here.
#[tauri::command]
pub async fn import_read_local(source: String) -> Result<Option<Vec<Vec<u8>>>, String> {
    tokio::task::spawn_blocking(move || read_local(&source)).await.map_err(|e| e.to_string())?
}

fn read_local(source: &str) -> Result<Option<Vec<Vec<u8>>>, String> {
    let config = dirs::config_dir();
    let mut parts: Vec<Vec<u8>> = Vec::new();
    match source {
        "putty" => {
            #[cfg(windows)]
            parts.extend(reg_export(r"HKCU\Software\SimonTatham\PuTTY\Sessions")?);
            #[cfg(not(windows))]
            if let Some(home) = dirs::home_dir() {
                parts.extend(putty_unix_sessions(&home.join(".putty").join("sessions"))?);
            }
        }
        "winscp" => {
            #[cfg(windows)]
            parts.extend(reg_export(r"HKCU\Software\Martin Prikryl\WinSCP 2")?);
            if parts.is_empty() {
                if let Some(dir) = &config {
                    parts.extend(read_file(&dir.join("WinSCP.ini"))?);
                }
            }
        }
        "mobaxterm" => {
            let mut inis = Vec::new();
            if let Some(dir) = &config {
                inis.push(dir.join("MobaXterm").join("MobaXterm.ini"));
            }
            if let Some(docs) = dirs::document_dir() {
                inis.push(docs.join("MobaXterm").join("MobaXterm.ini"));
            }
            for ini in inis {
                if let Some(bytes) = read_file(&ini)? {
                    parts.push(bytes);
                    break;
                }
            }
            #[cfg(windows)]
            if !parts.is_empty() {
                // Passwords and credentials of the installed edition.
                parts.extend(reg_export(r"HKCU\Software\Mobatek\MobaXterm")?);
            }
        }
        "mremoteng" => {
            if let Some(dir) = &config {
                parts.extend(read_file(&dir.join("mRemoteNG").join("confCons.xml"))?);
            }
        }
        "mremote" => {
            if let Some(dir) = &config {
                parts.extend(read_file(&dir.join("Felix_Deimel").join("mRemote").join("confCons.xml"))?);
            }
        }
        "tabby" => {
            if let Some(dir) = &config {
                parts.extend(read_file(&dir.join("tabby").join("config.yaml"))?);
            }
        }
        _ => return Err(format!("Unknown import source: {source}")),
    }
    Ok(if parts.is_empty() { None } else { Some(parts) })
}

fn read_file(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => {
            if m.len() > MAX_BYTES {
                return Err(format!("{} is too large", path.display()));
            }
            std::fs::read(path).map(Some).map_err(|e| format!("{}: {e}", path.display()))
        }
        _ => Ok(None),
    }
}

/// PuTTY on Linux and macOS: one file per session, named by the session's
/// %-escaped name, of `Key=Value` lines. Given back as one INI text with a
/// `[Sessions\name]` section each, the shape of a registry export.
#[cfg_attr(windows, allow(dead_code))]
fn putty_unix_sessions(dir: &Path) -> Result<Option<Vec<u8>>, String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(None) };
    let mut files: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect();
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let Some(bytes) = read_file(&path)? else { continue };
        out.extend_from_slice(format!("[Sessions\\{name}]\n").as_bytes());
        out.extend_from_slice(&bytes);
        out.push(b'\n');
    }
    Ok(if out.is_empty() { None } else { Some(out) })
}

/// A registry key exported as `reg export` writes it (UTF-16), or nothing
/// when the key is not there.
#[cfg(windows)]
fn reg_export(key: &str) -> Result<Option<Vec<u8>>, String> {
    use std::os::windows::process::CommandExt;
    let file = std::env::temp_dir().join(format!("reachercm-import-{}.reg", uuid::Uuid::new_v4()));
    let status = std::process::Command::new("reg")
        .arg("export")
        .arg(key)
        .arg(&file)
        .arg("/y")
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("reg export: {e}"))?;
    let bytes = if status.success() { read_file(&file) } else { Ok(None) };
    let _ = std::fs::remove_file(&file);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_becomes_json() {
        let v = import_yaml_to_json("profiles:\n  - type: ssh\n    name: web\n    options:\n      host: web1\n      port: 2222\n".into()).unwrap();
        assert_eq!(v["profiles"][0]["options"]["host"], "web1");
        assert_eq!(v["profiles"][0]["options"]["port"], 2222);
    }

    #[test]
    fn putty_unix_files_become_sections() {
        let dir = std::env::temp_dir().join(format!("reachercm-putty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("my%20box"), "HostName=box\nProtocol=ssh\n").unwrap();
        let text = String::from_utf8(putty_unix_sessions(&dir).unwrap().unwrap()).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(text.starts_with("[Sessions\\my%20box]\nHostName=box\n"));
    }

    #[test]
    fn unknown_source_is_an_error() {
        assert!(read_local("nope").is_err());
    }
}
