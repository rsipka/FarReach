//! Resolve the identity once, before either kube-client or Helm sees it.
use kube::config::Kubeconfig;

pub struct Selection {
    config: Kubeconfig,
}

impl Selection {
    pub fn new(yaml: &str, name: &str) -> Result<Self, String> {
        let source = Kubeconfig::from_yaml(yaml).map_err(|e| format!("Not a kubeconfig: {e}"))?;
        let mut context = source
            .contexts
            .into_iter()
            .find(|c| c.name == name)
            .ok_or("Unknown Kubernetes context")?;
        let ctx = context.context.as_mut().ok_or("Empty Kubernetes context")?;
        let mut cluster = source
            .clusters
            .into_iter()
            .find(|c| c.name == ctx.cluster)
            .ok_or("Unknown Kubernetes cluster")?;
        let endpoint = cluster.cluster.as_mut().ok_or("Empty Kubernetes cluster")?;
        // kube-rs preserves unmodeled fields. Do not pass them on to a second
        // parser (Helm uses client-go), or copy arbitrary extension metadata.
        context.other.clear();
        ctx.other.clear();
        ctx.extensions = None;
        cluster.other.clear();
        endpoint.other.clear();
        if let Some(extensions) = &mut endpoint.extensions {
            extensions.retain(|e| e.name == "client.authentication.k8s.io/exec");
        }
        let mut auth_infos = Vec::new();
        if let Some(user) = ctx.user.as_deref().filter(|u| !u.is_empty()) {
            let mut identity = source
                .auth_infos
                .into_iter()
                .find(|u| u.name == user)
                .ok_or("Unknown Kubernetes user")?;
            identity.other.clear();
            if let Some(auth) = &mut identity.auth_info {
                auth.other.clear();
                auth.extensions = None;
                if let Some(exec) = &mut auth.exec {
                    exec.other.clear();
                }
                if let Some(provider) = &mut auth.auth_provider {
                    provider.other.clear();
                }
            }
            auth_infos.push(identity);
        }
        Ok(Self {
            config: Kubeconfig {
                kind: Some("Config".into()),
                api_version: Some("v1".into()),
                current_context: Some(name.into()),
                contexts: vec![context],
                clusters: vec![cluster],
                auth_infos,
                ..Default::default()
            },
        })
    }

    pub fn server(&self) -> Result<&str, String> {
        self.config.clusters[0]
            .cluster
            .as_ref()
            .and_then(|c| c.server.as_deref())
            .ok_or_else(|| "Missing Kubernetes API server".into())
    }

    fn confirmation(&self, ssh_host: Option<&str>) -> Option<String> {
        let identity = self.config.auth_infos.first();
        let auth = identity.and_then(|u| u.auth_info.as_ref());
        let exec = auth.and_then(|a| a.exec.as_ref()).and_then(|e| e.command.as_deref());
        let provider = auth
            .and_then(|a| a.auth_provider.as_ref())
            .and_then(|p| p.config.get("cmd-path"))
            .map(String::as_str);
        let helpers: Vec<_> = [exec, provider].into_iter().flatten().map(label).collect();
        if ssh_host.is_none() && helpers.is_empty() {
            return None;
        }
        let context = label(&self.config.contexts[0].name);
        let mut message = format!("Kubernetes context {context}.");
        let location = match ssh_host {
            Some(host) => {
                let user = identity.map(|i| label(&i.name)).unwrap_or_else(|| "(anonymous)".into());
                message.push_str(&format!("\nRemote Helm will receive this context's configuration and credentials for {user} on SSH host {host}. Referenced credential files are read on that host."));
                host
            }
            None => "this computer",
        };
        if !helpers.is_empty() {
            message.push_str(&format!("\nThe kubeconfig can run {} on {location} with its configured arguments and environment. Continue only if you trust the entire selected configuration.", helpers.join(", ")));
        }
        Some(message)
    }

    pub async fn authorize<F, A>(self, ssh_host: Option<&str>, ask: F) -> Result<Kubeconfig, String>
    where
        F: FnOnce(String) -> A,
        A: std::future::Future<Output = bool>,
    {
        if let Some(message) = self.confirmation(ssh_host) {
            if !ask(message).await {
                return Err("Kubernetes credential access was declined".into());
            }
        }
        Ok(self.config)
    }
}

pub fn label(raw: &str) -> String {
    format!("\"{}\"", raw.escape_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"
apiVersion: v1
kind: Config
current-context: other
contexts:
- name: other
  context: {cluster: other, user: other}
- name: chosen
  context: {cluster: chosen, user: reader, namespace: staging}
clusters:
- name: other
  cluster: {server: 'https://other'}
- name: chosen
  cluster: {server: 'https://chosen', tls-server-name: chosen}
users:
- name: other
  user: {token: unrelated-secret}
- name: reader
  user: {token: selected-secret}
extensions: [{name: metadata, extension: {value: unrelated-secret}}]
"#;

    #[tokio::test]
    async fn remote_helm_receives_only_selected_credentials_after_confirmation() {
        let config = Selection::new(CONFIG, "chosen")
            .unwrap()
            .authorize(Some("jump.example"), |message| async move {
                assert!(message.contains("jump.example") && message.contains("reader") && message.contains("chosen"));
                assert!(!message.contains("secret"));
                true
            })
            .await
            .unwrap();
        let wire = serde_json::to_string(&config).unwrap();
        assert!(!wire.contains("other") && !wire.contains("unrelated-secret"));
        let value: serde_json::Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(value["current-context"], "chosen");
        assert_eq!(value["contexts"][0]["context"]["namespace"], "staging");
        assert_eq!(value["clusters"][0]["cluster"]["tls-server-name"], "chosen");
        assert_eq!(value["users"][0]["user"]["token"], "selected-secret");
        assert!(Selection::new(CONFIG, "chosen")
            .unwrap()
            .authorize(Some("jump.example"), |_| async { false })
            .await
            .is_err());
    }

    #[tokio::test]
    async fn helper_configuration_requires_fresh_consent_before_use() {
        for mechanisms in [
            "exec: {command: credential-helper, args: [private-argument]}",
            "auth-provider: {name: gcp, config: {cmd-path: credential-helper, cmd-args: private-argument}}",
        ] {
            let yaml = CONFIG.replace("token: selected-secret", mechanisms);
            for destination in [None, Some("jump.example")] {
                for allow in [false, true] {
                    let approved = Selection::new(&yaml, "chosen")
                        .unwrap()
                        .authorize(destination, |message| async move {
                            assert!(message.contains("credential-helper") && message.contains("chosen"));
                            assert!(message.contains(destination.unwrap_or("this computer")));
                            assert!(!message.contains("private-argument"));
                            allow
                        })
                        .await;
                    assert_eq!(approved.is_ok(), allow);
                    if let Ok(config) = approved {
                        assert!(serde_json::to_string(&config).unwrap().contains("private-argument"));
                    }
                }
            }
        }
    }
}
