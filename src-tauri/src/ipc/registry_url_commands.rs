//! Where the theme and recipe registries are fetched from.
//!
//! Both default to the registries the app ships with. A different address
//! can be saved in Settings > Plugins > Registries, for a registry of your
//! own or a mirror; it is kept in the settings vault (so it syncs like other
//! settings) and *Reset* goes back to the default. The plugin marketplace has
//! its own field in its panel (marketplace_commands.rs).
//!
//! Only https addresses are accepted. A registry index names the files to
//! download and the hashes they are checked against, and recipes are shell
//! scripts run on servers, so the index itself must not be open to anyone
//! between this computer and the registry.

use serde::Serialize;

use crate::ipc::settings_commands::{delete_setting_by_key, read_setting_by_key, write_setting_by_key};
use crate::recipe::registry::DEFAULT_RECIPES_URL;
use crate::state::AppState;
use crate::theme::DEFAULT_THEMES_URL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Registry {
    Themes,
    Recipes,
}

impl Registry {
    fn parse(kind: &str) -> Result<Self, String> {
        match kind {
            "themes" => Ok(Self::Themes),
            "recipes" => Ok(Self::Recipes),
            other => Err(format!("Unknown registry: {other}")),
        }
    }

    /// Settings-vault key for the saved address.
    fn key(self) -> &'static str {
        match self {
            Self::Themes => "theme_registry_url",
            Self::Recipes => "recipe_registry_url",
        }
    }

    fn default_url(self) -> &'static str {
        match self {
            Self::Themes => DEFAULT_THEMES_URL,
            Self::Recipes => DEFAULT_RECIPES_URL,
        }
    }
}

/// An address the registry may be fetched from: https, with a host.
pub(crate) fn check_url(url: &str) -> Result<String, String> {
    let url = url.trim();
    let parsed = reqwest::Url::parse(url).map_err(|_| "The registry address is not a valid URL".to_string())?;
    if parsed.scheme() != "https" {
        return Err("The registry address must start with https://".into());
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return Err("The registry address has no host".into());
    }
    Ok(url.to_string())
}

/// The saved address, if there is one and the vault is open to read it.
async fn saved_url(state: &AppState, registry: Registry) -> Option<String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return None;
    }
    let url = read_setting_by_key(&manager, registry.key()).await?;
    match check_url(&url) {
        Ok(url) => Some(url),
        Err(e) => {
            tracing::warn!("Saved {:?} registry address ignored: {e}", registry);
            None
        }
    }
}

/// The address to fetch `registry` from: the saved one, else the default.
pub(crate) async fn effective_url(state: &AppState, registry: Registry) -> String {
    saved_url(state, registry).await.unwrap_or_else(|| registry.default_url().to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryUrl {
    /// The address in effect.
    pub url: String,
    /// The address the app ships with.
    pub default_url: String,
    /// Whether `url` is the default.
    pub is_default: bool,
}

#[tauri::command]
pub async fn registry_url_get(state: tauri::State<'_, AppState>, kind: String) -> Result<RegistryUrl, String> {
    let registry = Registry::parse(&kind)?;
    let url = effective_url(&state, registry).await;
    Ok(RegistryUrl {
        is_default: url == registry.default_url(),
        default_url: registry.default_url().to_string(),
        url,
    })
}

/// Save a different address. Needs the vault unlocked, as the address is
/// kept in the settings vault.
#[tauri::command]
pub async fn registry_url_set(state: tauri::State<'_, AppState>, kind: String, url: String) -> Result<(), String> {
    let registry = Registry::parse(&kind)?;
    let url = check_url(&url)?;
    let mut manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Unlock the vault to save a registry address".into());
    }
    if url == registry.default_url() {
        return delete_setting_by_key(&mut manager, registry.key()).await;
    }
    write_setting_by_key(&mut manager, registry.key(), &url).await
}

/// Go back to the default address.
#[tauri::command]
pub async fn registry_url_reset(state: tauri::State<'_, AppState>, kind: String) -> Result<(), String> {
    let registry = Registry::parse(&kind)?;
    let mut manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("Unlock the vault to change a registry address".into());
    }
    delete_setting_by_key(&mut manager, registry.key()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_with_a_host() {
        assert_eq!(check_url(" https://example.org/themes.json ").unwrap(), "https://example.org/themes.json");
        assert!(check_url("http://example.org/themes.json").is_err());
        assert!(check_url("file:///etc/passwd").is_err());
        assert!(check_url("https://").is_err());
        assert!(check_url("not a url").is_err());
    }

    #[test]
    fn defaults_are_unchanged() {
        assert_eq!(Registry::parse("themes").unwrap().default_url(), DEFAULT_THEMES_URL);
        assert_eq!(Registry::parse("recipes").unwrap().default_url(), DEFAULT_RECIPES_URL);
        assert!(DEFAULT_THEMES_URL.contains("alexandrosnt/reach-themes-registry"));
        assert!(DEFAULT_RECIPES_URL.contains("alexandrosnt/reach-recipes-registry"));
        assert!(Registry::parse("plugins").is_err());
    }
}
