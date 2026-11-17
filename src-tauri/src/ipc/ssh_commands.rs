use crate::state::AppState;
use crate::ssh::client::{AuthParams, ConnectionInfo, JumpHostParams, KeySource, SshManager, exec_on_connection_with_exit_code_timeout};
use crate::plugin::hooks;

/// Parameters for a jump host received from the frontend.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JumpHostConnectParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: String,
    pub password: Option<String>,
    pub key_path: Option<String>,
    pub key_passphrase: Option<String>,
    /// An imported key, instead of a path. See `ssh_key_import`.
    pub key_id: Option<String>,
    /// Use the saved session's password manager entry for this hop's
    /// password; wins over `password`. Only a signal: the entry itself is
    /// read from the stored session (see `ssh_connect`). Sent as
    /// `passwordRef` (`password_ref` is accepted too).
    #[serde(default, alias = "password_ref")]
    pub password_ref: Option<crate::pwmanager::SecretRef>,
    /// The same for the key passphrase; wins over `key_passphrase`. Sent as
    /// `keyPassphraseRef` (or `key_passphrase_ref`).
    #[serde(default, alias = "key_passphrase_ref")]
    pub key_passphrase_ref: Option<crate::pwmanager::SecretRef>,
}

/// Which secret of a login a password manager reference stands in for, if
/// the login method uses one: the session slot's field name (see
/// `session_commands::ref_slots`).
fn ref_field(auth_method: &str, has_password_ref: bool, has_passphrase_ref: bool) -> Option<&'static str> {
    match auth_method {
        "password" if has_password_ref => Some("password"),
        "key" if has_passphrase_ref => Some("passphrase"),
        _ => None,
    }
}

/// The slots of the saved session whose password manager entries this
/// connect asks for. The references the webview sends say only that:
/// which entry is used, and where it goes, is read from the stored session.
pub(crate) fn wanted_slots(
    auth_method: &str,
    has_password_ref: bool,
    has_passphrase_ref: bool,
    jump_chain: Option<&[JumpHostConnectParams]>,
    proxy: Option<&crate::state::ProxyConfig>,
) -> Vec<String> {
    let mut out: Vec<String> = ref_field(auth_method, has_password_ref, has_passphrase_ref).map(String::from).into_iter().collect();
    let jumps = jump_chain.unwrap_or_default();
    for (i, j) in jumps.iter().enumerate() {
        if let Some(field) = ref_field(&j.auth_method, j.password_ref.is_some(), j.key_passphrase_ref.is_some()) {
            out.push(format!("jump:{i}:{field}"));
        }
    }
    // The proxy is used only without jump hosts.
    if jumps.is_empty() && proxy.is_some_and(|p| p.password_ref.is_some()) {
        out.push("proxy".to_string());
    }
    out
}

/// Where a proxy goes, without its credentials.
fn proxy_endpoint(p: &crate::state::ProxyConfig) -> (&str, &str, u16, Option<&str>) {
    (p.proxy_type.as_str(), p.host.as_str(), p.port, p.username.as_deref().filter(|u| !u.is_empty()))
}

/// The connect must go where the saved session goes for its password
/// manager entries to be sent: the same server, user, jump hosts and proxy.
/// Their approvals were given for exactly that (`approval_context`).
pub(crate) fn check_matches_session(
    stored: &crate::state::SessionConfig,
    host: &str,
    port: u16,
    username: &str,
    jump_chain: Option<&[JumpHostConnectParams]>,
    proxy: Option<&crate::state::ProxyConfig>,
) -> Result<(), String> {
    let jumps: Vec<(&str, u16, &str)> =
        jump_chain.unwrap_or_default().iter().map(|j| (j.host.as_str(), j.port, j.username.as_str())).collect();
    let stored_jumps: Vec<(&str, u16, &str)> =
        stored.jump_chain.iter().flatten().map(|j| (j.host.as_str(), j.port, j.username.as_str())).collect();
    let same = stored.kind == crate::state::SessionKind::Ssh
        && stored.host == host
        && stored.port == port
        && stored.username == username
        && jumps == stored_jumps
        && proxy.map(proxy_endpoint) == stored.proxy.as_ref().map(proxy_endpoint);
    if same {
        Ok(())
    } else {
        Err(format!(
            "This connection does not match the saved session '{}', so its password manager entries are not used. Reload the session list and connect again.",
            stored.name
        ))
    }
}

/// The reference in one slot of the stored session.
fn stored_ref<'a>(stored: &'a crate::state::SessionConfig, slot: &str) -> Result<&'a crate::pwmanager::SecretRef, String> {
    crate::ipc::session_commands::ref_at(stored, slot).ok_or_else(|| {
        format!("The saved session '{}' has no password manager entry for this login any more. Reload the session list and connect again.", stored.name)
    })
}

/// What a login may try, and nothing else. A key session offers its key, a
/// password session its password, an agent session the agent's keys. The
/// agent's keys are offered to a key session only when the user turned that
/// on for it (`try_agent_keys`).
///
/// Every refused offer counts against the server: OpenSSH's MaxAuthTries
/// (6) and PerSourcePenalties, fail2ban and firewalls block the address
/// after a few, and every key offered tells the server who you are. Offering
/// all the agent's keys behind a refused session key was guessing, and it
/// also hid a session whose own key no longer worked (it then failed on a
/// phone, which has no agent).
pub(crate) fn build_auth(
    auth_method: &str,
    password: Option<String>,
    key_source: Option<KeySource>,
    key_passphrase: Option<String>,
    try_agent_keys: bool,
) -> Result<AuthParams, String> {
    let mut auth = AuthParams { allow_agent: auth_method == "agent", ..Default::default() };
    match auth_method {
        "password" => {
            auth.password = Some(password.ok_or("Password required for password auth")?);
        }
        "key" => {
            auth.key = Some(crate::ssh::client::KeyAuth {
                source: key_source
                    .ok_or("Key auth needs either a key file or an imported key")?,
                passphrase: key_passphrase,
            });
            auth.allow_agent = try_agent_keys;
        }
        "agent" => {}
        _ => return Err(format!("Unknown auth method: {}", auth_method)),
    }
    Ok(auth)
}

/// Work out where a key comes from. An imported key wins over a path: it is
/// the one the user picked most recently, and it is the one that works on a
/// machine with no key files.
pub(crate) async fn resolve_key_source(
    state: &AppState,
    key_id: Option<String>,
    key_path: Option<String>,
) -> Result<(Option<KeySource>, Option<String>), String> {
    if let Some(id) = key_id.filter(|i| !i.is_empty()) {
        let resolved = crate::ipc::sshkey_commands::resolve(state, &id).await?;
        return Ok((
            Some(KeySource::Material(resolved.private_key)),
            resolved.passphrase,
        ));
    }
    Ok((
        key_path
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .map(KeySource::Path),
        None,
    ))
}

#[tauri::command]
#[expect(clippy::too_many_arguments, reason = "a Tauri command takes each argument from the frontend's invoke by name")]
pub async fn ssh_connect(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    host: String,
    port: u16,
    username: String,
    auth_method: String,
    password: Option<String>,
    key_path: Option<String>,
    key_passphrase: Option<String>,
    key_id: Option<String>,
    cols: u16,
    rows: u16,
    jump_chain: Option<Vec<JumpHostConnectParams>>,
    proxy: Option<crate::state::ProxyConfig>,
    shell: Option<String>,
    inject_colors: Option<bool>,
    show_login_message: Option<bool>,
    session_log: Option<crate::ssh::session_log::SessionLogConfig>,
    try_agent_keys: Option<bool>,
    ssh_options: Option<crate::ssh::sshconf::session::SshOptions>,
    password_ref: Option<crate::pwmanager::SecretRef>,
    key_passphrase_ref: Option<crate::pwmanager::SecretRef>,
    session_id: Option<String>,
    tmux: Option<crate::ssh::tmux::TmuxStart>,
) -> Result<String, String> {
    // Boxed: Tauri builds a command's future on the main thread and passes it
    // by value before spawning it, and the main thread's stack is small
    // (1 MB on Windows). Only a pointer is left there this way.
    Box::pin(async move {
        // Only the approvals of the person connecting count: a session synced
        // from a vault others can write to cannot approve its own commands.
        let mut ssh_options = ssh_options.map(|o| o.effective());
        // Nothing is typed into the user's shell unless the caller asks for it
        // (Settings → Appearance): a caller that does not say gets no setup. The
        // login message is shown unless asked otherwise.
        // tmux: the channel runs the tmux start script instead of the login
        // shell (which becomes the shell of a new tmux window). Nothing is
        // typed into it: the init would land in a tmux window, again on
        // every reattach.
        // Ask mode: the shell stays as it is until the picker answers, once
        // logged in (see `ssh::tmux::Ask`).
        let tmux_ask = tmux.as_ref().filter(|t| t.ask).map(|t| crate::ssh::tmux::Ask {
            id: id.clone(),
            start: t.clone(),
            shell: shell.clone(),
            app: app.clone(),
            host: host.clone(),
            port,
        });
        let shell = match &tmux {
            Some(t) if !t.ask => Some(crate::ssh::tmux::start_command(t, shell.as_deref())?),
            _ => shell,
        };
        let login = crate::ssh::client::LoginOptions {
            inject_colors: inject_colors.unwrap_or(false) && tmux.is_none(),
            show_login_message: show_login_message.unwrap_or(true),
            log: session_log,
            tmux_ask,
        };
        tracing::info!(
            "ssh_connect IPC: id={}, host={}, port={}, user={}, auth_method='{}', has_key_path={}, has_password={}, has_passphrase={}, has_password_ref={}, has_passphrase_ref={}, has_proxy={}, has_jump={}",
            id, host, port, username, auth_method,
            key_path.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
            password.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
            key_passphrase.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
            password_ref.is_some(),
            key_passphrase_ref.is_some(),
            proxy.is_some(),
            jump_chain.as_ref().map(|c| c.len()).unwrap_or(0),
        );
        // Secrets held by a password manager are fetched now, for the target,
        // every jump host and the proxy, before anything connects. The proxy is
        // used only without a jump chain.
        //
        // Which entries, and where they may go, come from the saved session as
        // stored, not from this call: the references sent here only say "use
        // the session's entry". The connect must match the session, and each
        // entry must be one the connecting person approved for it; otherwise
        // the error says which, for the session list to ask (see
        // `session_commands::require_trusted_refs`).
        let mut password = password;
        let mut key_passphrase = key_passphrase;
        let mut jump_chain = jump_chain;
        let mut proxy = proxy;
        let wanted = wanted_slots(&auth_method, password_ref.is_some(), key_passphrase_ref.is_some(), jump_chain.as_deref(), proxy.as_ref());
        if !wanted.is_empty() {
            let sid = crate::ipc::session_commands::require_session_id(session_id.as_deref())?;
            let stored = crate::ipc::session_commands::stored_session(&state, sid).await?;
            check_matches_session(&stored, &host, port, &username, jump_chain.as_deref(), proxy.as_ref())?;
            crate::ipc::session_commands::require_trusted_refs(&stored, None, &wanted)?;
            // The ssh_config settings decide where the login really goes
            // (HostName, ProxyJump): the stored session's, which the approvals
            // were given for.
            ssh_options = stored.ssh_options.clone().map(|o| o.effective());
            // One operation: if the manager's passphrase must be asked for, it
            // is asked once for all of these, not once per entry.
            let reason = crate::pwmanager::Reason::Connect(stored.name.clone());
            crate::pwmanager::with_prompt(&app, reason, async {
                if let Some(field) = ref_field(&auth_method, password_ref.is_some(), key_passphrase_ref.is_some()) {
                    let secret = crate::pwmanager::resolve_to_string(&state, stored_ref(&stored, field)?).await?;
                    if field == "password" { password = Some(secret) } else { key_passphrase = Some(secret) }
                }
                for (i, j) in jump_chain.iter_mut().flatten().enumerate() {
                    if let Some(field) = ref_field(&j.auth_method, j.password_ref.is_some(), j.key_passphrase_ref.is_some()) {
                        let secret = crate::pwmanager::resolve_to_string(&state, stored_ref(&stored, &format!("jump:{i}:{field}"))?).await?;
                        if field == "password" { j.password = Some(secret) } else { j.key_passphrase = Some(secret) }
                    }
                }
                if wanted.iter().any(|s| s == "proxy") {
                    if let Some(p) = proxy.as_mut() {
                        p.password = Some(crate::pwmanager::resolve_to_string(&state, stored_ref(&stored, "proxy")?).await?);
                    }
                }
                Ok::<(), String>(())
            })
            .await?;
        }
        if let Some(t) = &tmux {
            tracing::info!("ssh_connect: in tmux session '{}'", t.name);
        } else if let Some(s) = shell.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            tracing::info!("ssh_connect: per-session login shell override = '{}'", s);
        }
        // A key stored in the vault brings its own passphrase; one typed into the
        // connect dialog still wins, so a saved passphrase that went stale can be
        // corrected without editing the key.
        let (key_source, stored_passphrase) =
            resolve_key_source(&state, key_id, key_path).await?;
        let key_passphrase = key_passphrase.filter(|p| !p.is_empty()).or(stored_passphrase);
        let auth = build_auth(&auth_method, password, key_source, key_passphrase, try_agent_keys.unwrap_or(false))?;

        // Establish the connection WITHOUT holding the global ssh_manager lock. The
        // handshake/auth/shell setup can take up to the connect timeout (longer if a
        // host hangs); holding the lock across it would block ssh_send / ssh_resize /
        // ssh_disconnect on every other live connection. We lock only afterwards,
        // briefly, to register the finished connection (a single HashMap insert).
        // ssh_config settings for the target: resolved here, before connecting.
        let mut target_opts: crate::ssh::client::HopOptions =
            crate::ssh::sshconf::session::plan_for(ssh_options.as_ref(), &host, port, &username, false).into();
        // LogLevel/LogVerbose: everything logged while connecting is collected
        // for the terminal, and shown with the error if the connection fails.
        let connlog = target_opts.log.as_ref().map(|(level, verbose)| crate::ssh::connlog::ConnLog::new(level.as_deref(), verbose));
        target_opts.connlog = connlog.clone();
        let span = connlog.as_ref().map_or_else(tracing::Span::none, |l| l.span());
        let connected: Result<_, String> = tracing::Instrument::instrument(async {
        let conn = if let Some(chain) = jump_chain {
            if chain.is_empty() {
                // No jump hosts, connect directly
                SshManager::connect(&id, &host, port, &username, auth, cols, rows, app.clone(), proxy, shell, login, target_opts)
                    .await
                    .map_err(|e| e.to_string())?
            } else {
                // Resolve every jump host's key before the closure below, which
                // cannot await.
                let mut jump_key_sources: Vec<Option<(Option<KeySource>, Option<String>)>> =
                    Vec::with_capacity(chain.len());
                for j in &chain {
                    jump_key_sources.push(Some(
                        resolve_key_source(&state, j.key_id.clone(), j.key_path.clone()).await?,
                    ));
                }

                // Build jump host params
                let jump_params: Result<Vec<JumpHostParams>, String> = chain
                    .into_iter()
                    .map(|j| {
                        let (jsource, jstored) = jump_key_sources
                            .remove(0)
                            .ok_or_else(|| "Jump host key could not be resolved".to_string())?;
                        let jauth = build_auth(
                            &j.auth_method,
                            j.password,
                            jsource,
                            j.key_passphrase.filter(|p| !p.is_empty()).or(jstored),
                            false,
                        )?;
                        let opts = crate::ssh::sshconf::session::plan_for(ssh_options.as_ref(), &j.host, j.port, &j.username, true).into();
                        Ok(JumpHostParams {
                            host: j.host,
                            port: j.port,
                            username: j.username,
                            auth: jauth,
                            opts,
                        })
                    })
                    .collect();

                SshManager::connect_via_jump(
                    &id,
                    &host,
                    port,
                    &username,
                    auth,
                    jump_params?,
                    cols,
                    rows,
                    app.clone(),
                    shell,
                    login,
                    target_opts,
                )
                .await
                .map_err(|e| e.to_string())?
            }
        } else {
            SshManager::connect(&id, &host, port, &username, auth, cols, rows, app.clone(), proxy, shell, login, target_opts)
                .await
                .map_err(|e| e.to_string())?
        };
        Ok(conn)
        }, span).await;
        let conn = match connected {
            Ok(c) => c,
            Err(e) => {
                let Some(cl) = connlog else { return Err(e) };
                crate::ssh::connlog::close(&cl);
                let lines = cl.take();
                return Err(if lines.is_empty() { e } else { format!("{e}\n\n{}", lines.join("\n")) });
            }
        };

        // Register the finished connection under a brief lock, released immediately
        // (before plugin hooks, which may themselves need the lock).
        let info = state.ssh_manager.lock().await.register(conn);

        let connection_id = info.id.clone();

        // Fire-and-forget: a slow or hung plugin hook must not block the IPC
        // return. dispatch_hook applies a per-hook timeout internally.
        let hook = hooks::session_connected(&connection_id, &host, &username);
        let plugin_mgr = state.plugin_manager.clone();
        let app_for_hook = app.clone();
        tokio::spawn(async move {
            let mut mgr = plugin_mgr.lock().await;
            mgr.dispatch_hook(&hook, Some(&app_for_hook)).await;
        });

        Ok(connection_id)
    })
    .await
}

#[tauri::command]
pub async fn ssh_send(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    crate::history::input(&connection_id, &data);
    let manager = state.ssh_manager.lock().await;
    manager.send_data(&connection_id, &data).map_err(|e| e.to_string())
}

/// Called by the terminal once its data listener is attached: flushes any
/// buffered remote output (motd/banner) and switches to live streaming.
#[tauri::command]
pub async fn ssh_ready(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let manager = state.ssh_manager.lock().await;
    manager.mark_ready(&connection_id).map_err(|e| e.to_string())
}

/// Where session logs go when Settings leaves the folder empty, shown there
/// as the placeholder.
#[tauri::command]
pub fn session_log_default_dir() -> String {
    crate::ssh::session_log::default_dir().display().to_string()
}

/// The frontend's host-key verification dialog reports the user's decision,
/// un-parking the SSH handshake that's waiting on `check_server_key`.
#[tauri::command]
pub fn ssh_hostkey_response(prompt_id: String, accept: bool) {
    crate::ssh::client::resolve_hostkey_prompt(&prompt_id, accept);
}

/// The answer to a login question (password, passphrase, keyboard-interactive,
/// yes/no); `None` when the user cancelled.
#[tauri::command]
pub fn ssh_auth_prompt_response(prompt_id: String, answers: Option<Vec<String>>) {
    crate::ssh::prompt::resolve(&prompt_id, answers);
}

#[tauri::command]
pub async fn ssh_resize(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let manager = state.ssh_manager.lock().await;
    manager.resize(&connection_id, cols, rows).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_disconnect(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let mut manager = state.ssh_manager.lock().await;
    manager.disconnect(&connection_id).map_err(|e| e.to_string())?;
    drop(manager);
    // Transfers on this connection end with it, rather than being left
    // waiting on a channel that is gone (#161).
    crate::ipc::sftp_commands::cancel_transfers_on(&state, &connection_id);

    // Fire-and-forget hook dispatch (see ssh_connect for rationale).
    let hook = hooks::session_disconnected(&connection_id);
    let plugin_mgr = state.plugin_manager.clone();
    let app_for_hook = app.clone();
    tokio::spawn(async move {
        let mut mgr = plugin_mgr.lock().await;
        mgr.dispatch_hook(&hook, Some(&app_for_hook)).await;
    });

    Ok(())
}

#[tauri::command]
pub async fn ssh_list_connections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectionInfo>, String> {
    let manager = state.ssh_manager.lock().await;
    Ok(manager.list_connections())
}

/// OS probes answer at once or not at all.
const OS_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Detect the remote operating system. Probes, in order:
/// 1. `/etc/os-release` (Linux; MSYS2/Cygwin on Windows ship one too)
/// 2. `uname -s` (macOS, BSDs; Git Bash on Windows answers `MINGW64_NT-10.0`)
/// 3. `cmd /c ver` (Windows, whatever its default shell)
/// Returns a lowercase distro ID (e.g. "debian", "ubuntu", "alpine"),
/// "windows", or "unknown" when nothing answered: a router or a restricted
/// shell is not Linux, and guessing so put a penguin on it for good.
/// (Ported from the ReachQ fork by CPTProgrammer.)
#[tauri::command]
pub async fn ssh_detect_os(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<String, String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };

    // Exit codes matter: cmd.exe on Windows answers `cat` with an error on
    // stderr, and a shell that prints a banner must not pass for an answer.
    if let Ok((out, _, 0)) = exec_on_connection_with_exit_code_timeout(&handle, "cat /etc/os-release", OS_PROBE_TIMEOUT).await {
        return Ok(os_from_os_release(&out));
    }
    if let Ok((out, _, 0)) = exec_on_connection_with_exit_code_timeout(&handle, "uname -s", OS_PROBE_TIMEOUT).await {
        if let Some(os) = os_from_uname(&out) {
            return Ok(os);
        }
    }
    if let Ok((out, _, 0)) = exec_on_connection_with_exit_code_timeout(&handle, "cmd /c ver", OS_PROBE_TIMEOUT).await {
        if out.to_lowercase().contains("windows") {
            return Ok("windows".to_string());
        }
    }
    Ok("unknown".to_string())
}

/// The `ID=` of an os-release file (not `ID_LIKE=`). MSYS2, MinGW and Cygwin
/// ship one on a Windows host; WSL's is its real distro, which is right.
fn os_from_os_release(text: &str) -> String {
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("ID=") {
            let id = rest.trim().trim_matches(|c| c == '"' || c == '\'').to_lowercase();
            if id.is_empty() {
                break;
            }
            if id.contains("msys") || id.contains("mingw") || id.contains("cygwin") {
                return "windows".to_string();
            }
            return id;
        }
    }
    // An os-release without an ID is still a Linux system.
    "linux".to_string()
}

/// `uname -s`, lowercased. Git Bash, MSYS and Cygwin on Windows answer
/// `MINGW64_NT-10.0-19045`, `MSYS_NT-…` or `CYGWIN_NT-…`.
fn os_from_uname(text: &str) -> Option<String> {
    let os = text.lines().next()?.trim().to_lowercase();
    if os.is_empty() {
        return None;
    }
    if os.contains("_nt") {
        return Some("windows".to_string());
    }
    Some(os)
}

#[cfg(test)]
mod detect_os_tests {
    use super::*;

    #[test]
    fn os_release_gives_its_id() {
        let ubuntu = "NAME=\"Ubuntu\"\nID_LIKE=debian\nID=ubuntu\nVERSION_ID=\"24.04\"\n";
        assert_eq!(os_from_os_release(ubuntu), "ubuntu");
        assert_eq!(os_from_os_release("ID='alpine'\n"), "alpine");
        assert_eq!(os_from_os_release("NAME=Something\n"), "linux");
    }

    #[test]
    fn msys_and_cygwin_are_windows() {
        assert_eq!(os_from_os_release("ID=msys2\n"), "windows");
        assert_eq!(os_from_os_release("ID=cygwin\n"), "windows");
        assert_eq!(os_from_uname("MINGW64_NT-10.0-19045\n").as_deref(), Some("windows"));
        assert_eq!(os_from_uname("CYGWIN_NT-10.0\n").as_deref(), Some("windows"));
    }

    #[test]
    fn uname_gives_the_kernel() {
        assert_eq!(os_from_uname("Darwin\n").as_deref(), Some("darwin"));
        assert_eq!(os_from_uname("FreeBSD\n").as_deref(), Some("freebsd"));
        assert_eq!(os_from_uname("  \n"), None);
    }
}

/// The tmux start script for a WSL tab, which runs it with
/// `wsl.exe -e sh -c <script>` (see `ssh::tmux::start_script`).
#[tauri::command]
pub fn tmux_start_script(start: crate::ssh::tmux::TmuxStart) -> Result<String, String> {
    crate::ssh::tmux::start_script(&start, None)
}

/// What the tmux picker chose while connection `connection_id` opened,
/// once: `{ "name": "main" }`, `{ "name": null }` for no tmux, or null
/// when nothing was asked.
#[tauri::command]
pub fn ssh_tmux_chosen(connection_id: String) -> Option<serde_json::Value> {
    crate::ssh::tmux::take_chosen(&connection_id).map(|name| serde_json::json!({ "name": name }))
}

/// The server's tmux sessions, for a tab's "Switch tmux session".
#[tauri::command]
pub async fn ssh_tmux_list(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<crate::ssh::tmux::Listed>, String> {
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    let out = crate::ssh::client::exec_on_connection(&handle, crate::ssh::tmux::LIST_COMMAND).await.map_err(|e| e.to_string())?;
    Ok(crate::ssh::tmux::parse_list(&out))
}

/// Ends a tab's tmux session for good (the tab's own connection runs it),
/// with the sessions other tabs joined it as.
#[tauri::command]
pub async fn ssh_tmux_kill(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    name: String,
) -> Result<(), String> {
    let command = crate::ssh::tmux::kill_command(&name)?;
    let handle = {
        let manager = state.ssh_manager.lock().await;
        manager.get_handle(&connection_id).map_err(|e| e.to_string())?
    };
    crate::ssh::client::exec_on_connection(&handle, &command).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Inspect a key-file path so the UI can warn when the user picked the wrong
/// file (e.g. an OpenSSH public key) and suggest the right private key from the
/// same folder. Pure filesystem read — no connection or state required.
#[tauri::command]
pub fn inspect_key_file(path: String) -> crate::ssh::keyfile::KeyFileInfo {
    crate::ssh::keyfile::classify_path(&path)
}

#[cfg(test)]
mod auth_choice_tests {
    use super::*;

    fn key() -> Option<KeySource> {
        Some(KeySource::Material("k".into()))
    }

    /// Each kind of session offers what it names and nothing else, as PuTTY
    /// (userauth2-client.c filters Pageant down to the configured key) and
    /// OpenSSH with IdentitiesOnly do. No other key, no password on the side.
    #[test]
    fn a_session_offers_only_what_it_names() {
        let k = build_auth("key", Some("pw".into()), key(), None, false).unwrap();
        assert!(k.key.is_some() && !k.allow_agent && k.password.is_none());

        let p = build_auth("password", Some("pw".into()), None, None, true).unwrap();
        assert!(p.password.is_some() && !p.allow_agent && p.key.is_none());

        let a = build_auth("agent", None, None, None, false).unwrap();
        assert!(a.allow_agent && a.key.is_none() && a.password.is_none());
    }

    #[test]
    fn the_agent_backs_a_key_only_when_the_session_asks() {
        assert!(build_auth("key", None, key(), None, true).unwrap().allow_agent);
    }
}
