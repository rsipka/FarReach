//! virt-viewer connection files (`.vv`), as Proxmox VE, oVirt and
//! virt-manager hand them out: an INI file with a `[virt-viewer]` section.
//!
//! ```text
//! [virt-viewer]
//! type=spice
//! host=pvespiceproxy:6a3c...:101:node1::0f1e...
//! proxy=http://node1.example.com:3128
//! tls-port=61000
//! password=0123abcd...
//! host-subject=OU=PVE Cluster Node,O=Proxmox Virtual Environment,CN=node1.example.com
//! ca=-----BEGIN CERTIFICATE-----\nMIIF...\n-----END CERTIFICATE-----\n
//! title=VM 101 - web
//! delete-this-file=1
//! ```
//!
//! A Proxmox ticket is valid for about 30 seconds and once, so a file is
//! opened, not saved as a session.

use serde::Serialize;

/// What a .vv file says to connect to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VvFile {
    pub host: String,
    pub port: u16,
    pub tls_port: Option<u16>,
    pub password: String,
    pub ca: Option<String>,
    pub host_subject: Option<String>,
    pub proxy: Option<String>,
    pub title: Option<String>,
    /// The file asks to be deleted once read (Proxmox does: the ticket in
    /// it is spent).
    pub delete_after: bool,
}

/// The largest .vv file read; real ones are a few kilobytes.
pub const MAX_BYTES: u64 = 256 * 1024;

pub fn parse(text: &str) -> Result<VvFile, String> {
    let mut in_section = false;
    let mut found_section = false;
    let mut file = VvFile::default();
    let mut kind = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            in_section = line.eq_ignore_ascii_case("[virt-viewer]");
            found_section |= in_section;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, value) = (key.trim().to_ascii_lowercase(), value.trim());
        let port = |v: &str| v.parse::<u16>().ok().filter(|p| *p != 0);
        match key.as_str() {
            "type" => kind = Some(value.to_ascii_lowercase()),
            "host" => file.host = value.to_string(),
            "port" => file.port = port(value).unwrap_or(0),
            "tls-port" => file.tls_port = port(value),
            "password" => file.password = value.to_string(),
            "ca" => file.ca = Some(unescape(value)).filter(|v| !v.trim().is_empty()),
            "host-subject" => file.host_subject = Some(value.to_string()).filter(|v| !v.is_empty()),
            "proxy" => file.proxy = Some(value.to_string()).filter(|v| !v.is_empty()),
            "title" => file.title = Some(value.to_string()).filter(|v| !v.is_empty()),
            "delete-this-file" => file.delete_after = value == "1" || value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    if !found_section {
        return Err("Not a virt-viewer file: it has no [virt-viewer] section".into());
    }
    match kind.as_deref() {
        Some("spice") => {}
        Some(other) => return Err(format!("This file is for a {other} display; only SPICE ones can be opened here")),
        None => return Err("The file does not say which display it is for (type=spice)".into()),
    }
    if file.host.is_empty() {
        return Err("The file names no host".into());
    }
    if file.port == 0 && file.tls_port.is_none() {
        return Err("The file names no port".into());
    }
    Ok(file)
}

/// virt-viewer's `\n` (and `\\`) escapes, as in the `ca` key.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
