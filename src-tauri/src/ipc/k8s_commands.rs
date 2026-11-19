//! The Kubernetes workspace: clusters from a kubeconfig, directly or through
//! an SSH session, and their Helm releases. Every command checks the DevOps
//! switch first.

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use crate::db::types::Route;
use crate::devops::{self, Tool};
use crate::devops_store::{Cluster as SavedCluster, ClusterView, Environment};
use crate::k8s::credentials::{label, Selection};
use crate::k8s::helm::{self, Detail, HelmAction, Revision};
use crate::k8s::{self, ContextInfo, EventRow, Kind, ObjectRow, PodRow, WorkloadRow};
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contexts {
    pub contexts: Vec<ContextInfo>,
    pub current: Option<String>,
}

/// The contexts in a kubeconfig, without connecting to anything.
#[tauri::command]
pub fn k8s_contexts(yaml: String) -> Result<Contexts, String> {
    devops::require(Tool::Kubernetes)?;
    let (contexts, current) = k8s::contexts(&yaml)?;
    Ok(Contexts { contexts, current })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

async fn saved(state: &State<'_, AppState>, id: &str) -> Result<SavedCluster, String> {
    let mut vault = state.vault_manager.lock().await;
    let mut store = state.devops_store.lock().await;
    store.ensure_loaded(&mut vault).await?;
    store.cluster(id).cloned().ok_or_else(|| "That cluster is no longer saved".to_string())
}

#[tauri::command]
pub async fn k8s_clusters(state: State<'_, AppState>) -> Result<Vec<ClusterView>, String> {
    devops::require(Tool::Kubernetes)?;
    let mut vault = state.vault_manager.lock().await;
    let mut store = state.devops_store.lock().await;
    store.ensure_loaded(&mut vault).await?;
    Ok(store.clusters())
}

/// Save a cluster. A new one (empty id) needs `yaml`; editing one keeps its
/// stored kubeconfig unless a new one is given. The kubeconfig is never
/// sent back.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn k8s_cluster_save(
    state: State<'_, AppState>,
    id: String,
    name: String,
    yaml: Option<String>,
    context: String,
    route: Route,
    environment: Environment,
    read_only: bool,
    namespace: Option<String>,
) -> Result<ClusterView, String> {
    devops::require(Tool::Kubernetes)?;
    if name.trim().is_empty() {
        return Err("Give the cluster a name".into());
    }
    if matches!(route, Route::Live { .. }) {
        return Err("A terminal tab's connection ends with the tab; save the session instead".into());
    }
    let kubeconfig = match yaml.filter(|y| !y.trim().is_empty()) {
        Some(y) => y,
        None if !id.is_empty() => saved(&state, &id).await?.kubeconfig,
        None => return Err("Paste or choose a kubeconfig".into()),
    };
    let (list, _) = k8s::contexts(&kubeconfig)?;
    if !list.iter().any(|c| c.name == context) {
        return Err(format!("The kubeconfig has no context called {context}"));
    }
    let cluster = SavedCluster {
        id: if id.is_empty() { uuid::Uuid::new_v4().to_string() } else { id },
        name: name.trim().to_string(),
        kubeconfig,
        context,
        route,
        environment,
        read_only,
        namespace: namespace.filter(|n| !n.is_empty()),
        last_used_at: now_ms(),
    };
    // A changed setting (read-only, route) takes effect on the next open.
    let view = cluster.view();
    let mut vault = state.vault_manager.lock().await;
    let mut store = state.devops_store.lock().await;
    store.ensure_loaded(&mut vault).await?;
    store.save_cluster(cluster, &mut vault).await?;
    state.k8s.lock().await.close(&view.id);
    Ok(view)
}

#[tauri::command]
pub async fn k8s_cluster_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    devops::require(Tool::Kubernetes)?;
    let mut vault = state.vault_manager.lock().await;
    let mut store = state.devops_store.lock().await;
    store.ensure_loaded(&mut vault).await?;
    store.delete(&id, &mut vault).await?;
    state.k8s.lock().await.close(&id);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedCluster {
    pub key: String,
    pub version: String,
    pub namespace: String,
    pub read_only: bool,
}

async fn confirm_access(app: &tauri::AppHandle, message: String) -> bool {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    let (answer, result) = tokio::sync::oneshot::channel();
    app.dialog().message(message).title("Kubernetes credential access")
        .kind(MessageDialogKind::Warning).buttons(MessageDialogButtons::OkCancel)
        .show(move |allowed| { let _ = answer.send(allowed); });
    result.await.unwrap_or(false)
}

/// Connect to a saved cluster. With an SSH route the API server is reached
/// through it, and its certificate is still checked by name. The kubeconfig
/// and the read-only setting come from the vault.
#[tauri::command]
pub async fn k8s_open(app: tauri::AppHandle, state: State<'_, AppState>, cluster_id: String) -> Result<OpenedCluster, String> {
    devops::require(Tool::Kubernetes)?;
    let s = saved(&state, &cluster_id).await?;
    let selected = Selection::new(&s.kubeconfig, &s.context)?;
    let server = selected.server()?.to_owned();
    let config = selected.authorize(None, |m| confirm_access(&app, m)).await?;
    let forward = match s.route {
        Route::Direct => None,
        ref r => {
            let (host, port) = k8s::server_addr(&server)?;
            crate::ipc::db_commands::forward_route(&app, &state, r, host, port).await?
        }
    };
    let mut cluster = k8s::connect(config, forward).await?;
    cluster.read_only = s.read_only;
    let namespace = s.namespace.clone().unwrap_or_else(|| cluster.namespace.clone());
    let opened = OpenedCluster { key: s.id.clone(), version: cluster.version.clone(), namespace, read_only: s.read_only };
    {
        let mut vault = state.vault_manager.lock().await;
        let mut store = state.devops_store.lock().await;
        if store.cluster(&s.id) != Some(&s) {
            return Err("The saved cluster changed while connecting. Open it again.".into());
        }
        state.k8s.lock().await.insert(s.id.clone(), cluster);
        let _ = store.save_cluster(SavedCluster { last_used_at: now_ms(), ..s }, &mut vault).await;
    }
    Ok(opened)
}

#[tauri::command]
pub async fn k8s_close(state: State<'_, AppState>, key: String) -> Result<(), String> {
    state.k8s.lock().await.close(&key);
    Ok(())
}

async fn cluster(state: &State<'_, AppState>, key: &str) -> Result<std::sync::Arc<k8s::Cluster>, String> {
    devops::require(Tool::Kubernetes)?;
    state.k8s.lock().await.get(key)
}

/// An empty namespace means all of them.
fn ns(namespace: &Option<String>) -> Option<&str> {
    namespace.as_deref().filter(|n| !n.is_empty())
}

#[tauri::command]
pub async fn k8s_namespaces(state: State<'_, AppState>, key: String) -> Result<Vec<String>, String> {
    cluster(&state, &key).await?.namespaces().await
}

#[tauri::command]
pub async fn k8s_pods(state: State<'_, AppState>, key: String, namespace: Option<String>) -> Result<Vec<PodRow>, String> {
    cluster(&state, &key).await?.pods(ns(&namespace)).await
}

#[tauri::command]
pub async fn k8s_workloads(state: State<'_, AppState>, key: String, namespace: Option<String>) -> Result<Vec<WorkloadRow>, String> {
    cluster(&state, &key).await?.workloads(ns(&namespace)).await
}

#[tauri::command]
pub async fn k8s_objects(state: State<'_, AppState>, key: String, kind: Kind, namespace: Option<String>) -> Result<Vec<ObjectRow>, String> {
    cluster(&state, &key).await?.objects(kind, ns(&namespace)).await
}

#[tauri::command]
pub async fn k8s_events(state: State<'_, AppState>, key: String, namespace: Option<String>) -> Result<Vec<EventRow>, String> {
    cluster(&state, &key).await?.events(ns(&namespace)).await
}

#[tauri::command]
pub async fn k8s_get_yaml(state: State<'_, AppState>, key: String, kind: Kind, namespace: Option<String>, name: String) -> Result<String, String> {
    cluster(&state, &key).await?.get_yaml(kind, ns(&namespace), &name).await
}

#[tauri::command]
pub async fn k8s_replace_yaml(
    state: State<'_, AppState>,
    key: String,
    kind: Kind,
    namespace: Option<String>,
    name: String,
    yaml: String,
) -> Result<(), String> {
    cluster(&state, &key).await?.replace_yaml(kind, ns(&namespace), &name, &yaml).await
}

#[tauri::command]
pub async fn k8s_delete(state: State<'_, AppState>, key: String, kind: Kind, namespace: Option<String>, name: String) -> Result<(), String> {
    cluster(&state, &key).await?.delete(kind, ns(&namespace), &name).await
}

#[tauri::command]
pub async fn k8s_scale(state: State<'_, AppState>, key: String, kind: Kind, namespace: String, name: String, replicas: i32) -> Result<(), String> {
    cluster(&state, &key).await?.scale(kind, &namespace, &name, replicas).await
}

#[tauri::command]
pub async fn k8s_restart(state: State<'_, AppState>, key: String, kind: Kind, namespace: String, name: String) -> Result<(), String> {
    cluster(&state, &key).await?.restart(kind, &namespace, &name).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn k8s_logs(
    state: State<'_, AppState>,
    key: String,
    namespace: String,
    pod: String,
    container: Option<String>,
    stream_id: String,
    tail: Option<i64>,
    view: Option<crate::container::LogView>,
    on_data: Channel<String>,
) -> Result<(), String> {
    let c = cluster(&state, &key).await?;
    let view = view.unwrap_or(crate::container::LogView { tail: tail.unwrap_or(500).clamp(0, u32::MAX as i64) as u32, since_secs: None, timestamps: false });
    state.k8s.lock().await.follow_logs(stream_id, c, namespace, pod, container, view, on_data);
    Ok(())
}

/// The log files in a pod's container, for the file picker (#101).
#[tauri::command]
pub async fn k8s_log_files(
    state: State<'_, AppState>,
    key: String,
    namespace: String,
    pod: String,
    container: Option<String>,
) -> Result<Vec<crate::logfiles::FoundFile>, String> {
    cluster(&state, &key).await?.find_log_files(&namespace, &pod, container).await
}

/// Follow files inside a pod's container, as `tail -F`, until
/// [`k8s_logs_stop`]. Reading only: a read-only cluster allows it.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn k8s_logs_files(
    state: State<'_, AppState>,
    key: String,
    namespace: String,
    pod: String,
    container: Option<String>,
    stream_id: String,
    paths: Vec<String>,
    lines: Option<u32>,
    on_data: Channel<String>,
) -> Result<(), String> {
    let paths = crate::logfiles::check_paths(&paths)?;
    let c = cluster(&state, &key).await?;
    state.k8s.lock().await.follow_files(stream_id, c, namespace, pod, container, paths, lines.unwrap_or(500).min(100_000), on_data);
    Ok(())
}

#[tauri::command]
pub async fn k8s_logs_stop(state: State<'_, AppState>, stream_id: String) -> Result<(), String> {
    state.k8s.lock().await.stop_stream(&stream_id);
    Ok(())
}

#[tauri::command]
pub async fn k8s_helm_releases(state: State<'_, AppState>, key: String, namespace: Option<String>) -> Result<Vec<Revision>, String> {
    cluster(&state, &key).await?.releases(ns(&namespace)).await
}

#[tauri::command]
pub async fn k8s_helm_history(state: State<'_, AppState>, key: String, namespace: String, name: String) -> Result<Vec<Revision>, String> {
    cluster(&state, &key).await?.release_history(&namespace, &name).await
}

#[tauri::command]
pub async fn k8s_helm_detail(state: State<'_, AppState>, key: String, namespace: String, name: String, revision: i32) -> Result<Detail, String> {
    cluster(&state, &key).await?.release_detail(&namespace, &name, revision).await
}

async fn ensure_current(state: &State<'_, AppState>, snapshot: &SavedCluster) -> Result<(), String> {
    if saved(state, &snapshot.id).await? != *snapshot {
        return Err("The saved cluster changed. Start the Helm action again.".into());
    }
    Ok(())
}

/// Roll back or uninstall a release of a saved cluster with `helm`, on the
/// host the cluster is reached through (or this computer for a direct one),
/// with the cluster's own kubeconfig and context, so it acts as the same
/// identity on the same cluster.
#[tauri::command]
pub async fn k8s_helm_run(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    cluster_id: String,
    action: HelmAction,
    namespace: String,
    name: String,
    revision: Option<i32>,
) -> Result<String, String> {
    devops::require(Tool::Kubernetes)?;
    let s = saved(&state, &cluster_id).await?;
    if s.read_only {
        return Err("This cluster is read-only. Turn that off in its settings to make changes.".into());
    }
    let args = helm::arguments(action, &namespace, &name, revision, &s.context)?;
    let selected = Selection::new(&s.kubeconfig, &s.context)?;
    let o = match &s.route {
        Route::Direct => {
            let config = selected.authorize(None, |m| confirm_access(&app, m)).await?;
            ensure_current(&state, &s).await?;
            let input = serde_json::to_string(&config).map_err(|e| e.to_string())?;
            run_helm_here(&args, &input).await?
        }
        Route::Session { session_id } => {
            let session = crate::ipc::session_commands::session_get(state.clone(), session_id.clone()).await?;
            let destination = format!("{}@{}:{}", label(&session.username), label(&session.host), session.port);
            let pin = crate::ipc::session_commands::approval_context(&session);
            let config = selected.authorize(Some(&destination), |m| confirm_access(&app, m)).await?;
            let (h, _keep) = crate::ipc::db_commands::ssh_for_route_pinned(&app, &state, &s.route, Some(&pin))
                .await?.ok_or("Missing SSH session")?;
            ensure_current(&state, &s).await?;
            let cmd = helm::remote_command(&args);
            let input = serde_json::to_string(&config).map_err(|e| e.to_string())?;
            crate::container::Shell::Ssh(h)
                .run_with_input(&helm::with_stdin_kubeconfig(&cmd), Some(input.as_bytes()), 900)
                .await?
        }
        Route::Live { .. } => return Err("Remote Helm requires a saved SSH session".into()),
    };
    if !o.ok() {
        return Err(o.failure("helm"));
    }
    Ok(format!("{}{}", o.stdout, o.stderr))
}

/// Helm on this computer: the kubeconfig in a file of Reach's own, readable
/// by the user only, removed afterwards.
#[cfg(not(mobile))]
async fn run_helm_here(args: &[String], yaml: &str) -> Result<crate::container::Output, String> {
    use std::io::Write;
    let dir = crate::app_data_dir().join("tmp");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    file.write_all(yaml.as_bytes()).map_err(|e| e.to_string())?;
    let path = file.into_temp_path();
    let mut helm = tokio::process::Command::new("helm");
    helm.args(args).arg("--kubeconfig").arg(&path).kill_on_drop(true);
    #[cfg(windows)]
    helm.creation_flags(0x0800_0000);
    let output = tokio::time::timeout(std::time::Duration::from_secs(900), helm.output()).await
        .map_err(|_| "No answer from Helm within 900 seconds".to_string())?.map_err(|e| format!("Could not run Helm: {e}"))?;
    Ok(crate::container::Output {
        code: output.status.code().map(|code| code as u32),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[cfg(mobile)]
async fn run_helm_here(_args: &[String], _yaml: &str) -> Result<crate::container::Output, String> {
    Err("Local Helm is unavailable on a phone. Choose a saved SSH session.".into())
}
