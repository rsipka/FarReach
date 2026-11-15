use std::path::Path;

use libsql::{Builder, Database};

use crate::vault::error::VaultError;

/// Sync configuration for Turso.
pub struct SyncConfig {
    pub sync_url: String,
    pub auth_token: String,
}

/// Whether a sync URL may be used. A token goes with every request, so only
/// an encrypted scheme is accepted, except to this machine itself, where a
/// test server or a local tunnel may speak plain HTTP.
pub fn check_sync_url(url: &str) -> Result<(), VaultError> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    let rest = ["http://", "ws://"].iter().find_map(|s| lower.strip_prefix(s));
    let Some(rest) = rest else {
        return Ok(());
    };
    if is_loopback_host(rest) {
        return Ok(());
    }
    Err(VaultError::SyncError(
        "This sync URL is not encrypted (http://). Use libsql:// or https:// so the access token is not sent in the clear.".to_string(),
    ))
}

/// The host at the start of `authority/path`, judged loopback or not.
pub(crate) fn is_loopback_host(authority_and_path: &str) -> bool {
    let authority = authority_and_path.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or("");
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Create a database - local-only or remote (Turso).
/// Note: Embedded replicas have issues on Windows (https://github.com/tursodatabase/libsql/issues/2074)
/// So we use remote-only for synced vaults instead of embedded replicas.
pub async fn create_replica(
    path: &Path,
    sync_config: Option<&SyncConfig>,
) -> Result<Database, VaultError> {
    let db = match sync_config {
        Some(config) => {
            check_sync_url(&config.sync_url)?;
            // Remote-only connection to Turso (no local replica due to Windows bug)
            tracing::info!("Creating remote database connection: {}", config.sync_url);
            let connector = super::turso_tls::TursoConnector::new()
                .map_err(|e| VaultError::SyncError(format!("TLS setup failed: {}", e)))?;
            Builder::new_remote(
                config.sync_url.clone(),
                config.auth_token.clone(),
            )
            .connector(connector)
            .build()
            .await
            .map_err(|e| VaultError::SyncError(format!("Failed to connect to Turso: {}", e)))?
        }
        None => {
            // Local-only database
            Builder::new_local(path)
                .build()
                .await
                .map_err(|e| VaultError::DatabaseError(e.to_string()))?
        }
    };

    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_urls_pass() {
        assert!(check_sync_url("libsql://db-org.turso.io").is_ok());
        assert!(check_sync_url("https://db-org.turso.io").is_ok());
        assert!(check_sync_url("wss://db-org.turso.io").is_ok());
    }

    #[test]
    fn clear_text_only_to_this_machine() {
        assert!(check_sync_url("http://127.0.0.1:8080").is_ok());
        assert!(check_sync_url("http://localhost:8080/").is_ok());
        assert!(check_sync_url("HTTP://[::1]:8080").is_ok());
        assert!(check_sync_url("http://db-org.turso.io").is_err());
        assert!(check_sync_url("ws://10.0.0.5").is_err());
        assert!(check_sync_url("http://localhost.evil.com").is_err());
        assert!(check_sync_url("http://127.0.0.1@evil.com").is_err());
    }
}
