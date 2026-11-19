//! Project records live in the unlocked vault. A store borrows its manager for
//! one IPC operation, so neither records nor access survive a vault transition.
use secrecy::{ExposeSecret, SecretBox};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::{Mutex, MutexGuard};

use crate::vault::{SecretCategory, VaultManager, VaultType};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    name: String,
    path: String,
    description: String,
    created_at: String,
    last_opened_at: String,
}

pub struct ProjectStore<'a> {
    vault: MutexGuard<'a, VaultManager>,
    id: String,
    category: SecretCategory,
}

impl<'a> ProjectStore<'a> {
    pub async fn open(
        vault: &'a Mutex<VaultManager>,
        name: &str,
        category: &str,
    ) -> Result<Self, String> {
        let mut vault = vault.lock().await;
        if vault.is_locked() {
            return Err("Unlock the vault to access projects".into());
        }
        let id = if let Some(id) = vault.get_vault_id_by_name(name) {
            vault
                .open_vault(&id, None, None)
                .await
                .map_err(|e| e.to_string())?;
            vault.unlock_vault(&id).await.map_err(|e| e.to_string())?;
            id
        } else {
            vault
                .create_vault(name, VaultType::Private, None, None)
                .await
                .map_err(|e| e.to_string())?
                .id
        };
        Ok(Self {
            vault,
            id,
            category: SecretCategory::Custom(category.into()),
        })
    }

    pub fn schema_key(&self, project_id: &str) -> (String, String) {
        (self.id.clone(), project_id.to_owned())
    }

    pub async fn list(&self) -> Result<Vec<ProjectSummary>, String> {
        let category = self.category.to_string();
        let records = self
            .vault
            .read_secrets_in(&self.id, &[&category])
            .await
            .map_err(|e| e.to_string())?;
        let mut projects: Vec<ProjectSummary> = records
            .into_iter()
            .filter_map(|(_, bytes)| serde_json::from_slice(bytes.ok()?.expose_secret()).ok())
            .collect();
        projects.sort_by(|a, b| b.last_opened_at.cmp(&a.last_opened_at));
        Ok(projects)
    }

    pub async fn get<P: DeserializeOwned>(&self, id: &str) -> Result<P, String> {
        let bytes = self
            .vault
            .read_secret_in_category(&self.id, id, Some(&self.category.to_string()))
            .await
            .map_err(|e| e.to_string())?;
        serde_json::from_slice(bytes.expose_secret()).map_err(|e| e.to_string())
    }

    pub async fn add(&self, id: &str, name: &str, project: &impl Serialize) -> Result<(), String> {
        let bytes = serde_json::to_vec(project).map_err(|e| e.to_string())?;
        self.vault
            .create_secret_with_id(
                &self.id,
                id,
                name,
                self.category.clone(),
                SecretBox::new(Box::new(bytes)),
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Callers have read the typed record while holding this same vault guard.
    pub async fn update(&self, id: &str, project: &impl Serialize) -> Result<(), String> {
        let bytes = serde_json::to_vec(project).map_err(|e| e.to_string())?;
        self.vault
            .update_secret(&self.id, id, SecretBox::new(Box::new(bytes)))
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn remove(&self, id: &str) -> Result<(), String> {
        self.get::<ProjectSummary>(id).await?;
        self.vault
            .delete_secret(&self.id, id)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::manager::{ANSIBLE_PROJECTS_VAULT, TOFU_PROJECTS_VAULT};
    use serde_json::{json, Value};

    #[tokio::test]
    async fn project_access_follows_the_current_unlocked_vault() {
        let dir = std::env::temp_dir().join(format!("reach-projects-{}", uuid::Uuid::new_v4()));
        let vault = Mutex::new(VaultManager::new(dir.join("first")));
        let tools = [
            (ANSIBLE_PROJECTS_VAULT, "ansible_project"),
            (TOFU_PROJECTS_VAULT, "tofu_project"),
        ];
        for (name, category) in tools {
            assert!(ProjectStore::open(&vault, name, category).await.is_err());
        }
        vault
            .lock()
            .await
            .init_identity("first password")
            .await
            .unwrap();
        for (name, category) in tools {
            let store = ProjectStore::open(&vault, name, category).await.unwrap();
            let project = json!({"id":"one","name":"project","path":"/project","description":"",
                "createdAt":"yesterday","lastOpenedAt":"today","vaultPassword":"private",
                "encryption":{"enabled":true,"passphrase":"private"}});
            store.add("one", "project", &project).await.unwrap();
            let summaries = serde_json::to_value(store.list().await.unwrap()).unwrap();
            assert_eq!(
                summaries[0],
                json!({"id":"one","name":"project","path":"/project","description":"",
                "createdAt":"yesterday","lastOpenedAt":"today"})
            );
            assert_eq!(store.get::<Value>("one").await.unwrap(), project);
        }
        vault.lock().await.hold();
        for (name, category) in tools {
            assert!(ProjectStore::open(&vault, name, category).await.is_err());
        }
        assert!(vault.lock().await.unlock("first password").await.unwrap());
        for (name, category) in tools {
            let store = ProjectStore::open(&vault, name, category).await.unwrap();
            assert_eq!(store.list().await.unwrap().len(), 1);
            assert_eq!(store.get::<Value>("one").await.unwrap()["id"], "one");
        }
        *vault.lock().await = VaultManager::new(dir.join("second"));
        vault
            .lock()
            .await
            .init_identity("second password")
            .await
            .unwrap();
        for (name, category) in tools {
            let store = ProjectStore::open(&vault, name, category).await.unwrap();
            assert!(store.list().await.unwrap().is_empty());
            assert!(store.get::<Value>("one").await.is_err());
        }
        drop(vault);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
