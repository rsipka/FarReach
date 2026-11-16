//! 1Password, through its official command-line tool `op` (v2).
//!
//! Two ways to sign in (Settings → Password managers → 1Password):
//!
//! - **The 1Password app** (the default). With "Integrate with 1Password
//!   CLI" turned on in the app (Settings → Developer), `op` asks the running
//!   app, and the app asks the user to approve: Touch ID, Windows Hello, the
//!   system password or the account password. Reach stores nothing and asks
//!   nothing; the approval is the app's, and lasts as long as the app keeps
//!   it (ten minutes of inactivity by default). If several accounts are
//!   signed in to the app, the one to use is set as `account`.
//! - **A service account token**, stored by Reach in its vault like the
//!   Passbolt key and handed to `op` in `OP_SERVICE_ACCOUNT_TOKEN`. It sees
//!   only the vaults the service account was given.
//!
//! The rules for running the CLI are the ones in [`super::cli`] and
//! [`super::process`]: the binary must be `op` (`op.exe`) on this computer
//! and changeable only by the user or the system; the child gets an empty
//! environment plus an allowlist, with every `OP_*` setting removed (above
//! all `OP_CONNECT_HOST` / `OP_CONNECT_TOKEN`, which would send the calls to
//! a Connect server instead, and `OP_SESSION_*`); no stdin, so a prompt
//! fails at once; a deadline, capped output, one of the shared CLI slots.
//!
//! Calls: `op item list --format json` for the picker (metadata only: `op`
//! never prints field values in a listing), and `op read
//! op://<vault id>/<item id>/password` at connect time. An entry is stored
//! as `<vault id>/<item id>`, both 1Password's 26-character ids, checked
//! before they become an argument.

use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::path::Path;
#[cfg(desktop)]
use std::time::Duration;

use serde::Deserialize;
use zeroize::Zeroizing;

use super::cli::{self, CliSpec};
use super::config::{OnePasswordAuth, OnePasswordConfig, OnePasswordSecrets};
use super::unlock::Passphrase;
use super::{sanitize_display, EntrySummary, Failure, PasswordManager, ProviderId, ProviderStatus};

pub(super) const NAME: &str = "1Password";

pub const INSTALL_HINT: &str = "Install the 1Password CLI (the `op` command): `brew install 1password-cli` on a Mac, or see https://developer.1password.com/docs/cli/get-started/. Then turn on Settings → Developer → Integrate with 1Password CLI in the 1Password app.";

/// What `op` inherits from Reach's environment. Beyond the usual (PATH, the
/// Windows system directories, temp and home, proxies): the user's name and
/// runtime folder, which the CLI uses to find the 1Password app it talks to.
pub const INHERITED_ENV: &[&str] = &[
    "PATH",
    "SystemRoot",
    "SYSTEMROOT",
    "windir",
    "SystemDrive",
    "TEMP",
    "TMP",
    "TMPDIR",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "HOME",
    "USER",
    "LOGNAME",
    "USERNAME",
    "XDG_CONFIG_HOME",
    "XDG_RUNTIME_DIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
];

/// `op`'s own settings, removed by name in case one ends up in the
/// allowlist. `OP_SESSION_<account>` has no fixed name; it is kept out by
/// the allowlist alone.
pub const OP_ENV_KEYS: &[&str] = &[
    "OP_SERVICE_ACCOUNT_TOKEN",
    "OP_CONNECT_HOST",
    "OP_CONNECT_TOKEN",
    "OP_ACCOUNT",
    "OP_DEBUG",
    "OP_FORMAT",
    "OP_CACHE",
    "OP_CONFIG_DIR",
    "OP_ISO_TIMESTAMPS",
    "OP_INCLUDE_ARCHIVE",
    "OP_BIOMETRIC_UNLOCK_ENABLED",
    "OP_RUN_NO_MASKING",
];

pub(crate) const SPEC: CliSpec = CliSpec {
    name: NAME,
    program: "op",
    file_names: &["op", "op.exe"],
    install_hint: INSTALL_HINT,
    inherited_env: INHERITED_ENV,
    removed_env: OP_ENV_KEYS,
};

/// The most a listing may print.
pub const STDOUT_CAP: usize = 32 * 1024 * 1024;
pub const STDERR_CAP: usize = 64 * 1024;
#[cfg(desktop)]
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
/// The longest error text shown.
const MAX_ERROR_CHARS: usize = 300;

/// The item kinds that hold a password field, as `op` names them. Others
/// (notes, cards, SSH keys, documents) are left out of the picker.
const PASSWORD_CATEGORIES: &[&str] = &["LOGIN", "PASSWORD", "DATABASE", "SERVER", "WIRELESS_ROUTER", "EMAIL_ACCOUNT"];

/// What one CLI call does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Version,
    List,
    /// Vault and item id, already checked by [`validate_entry_id`].
    Read { vault: String, item: String },
}

impl Op {
    fn args(&self) -> Vec<String> {
        match self {
            Op::Version => vec!["--version".into()],
            Op::List => vec!["item".into(), "list".into(), "--format".into(), "json".into()],
            Op::Read { vault, item } => {
                vec!["read".into(), "--no-newline".into(), format!("op://{vault}/{item}/password")]
            }
        }
    }
}

/// A 1Password id: 26 lower-case letters and digits.
fn is_op_id(s: &str) -> bool {
    s.len() == 26 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// A stored entry: `<vault id>/<item id>`. Anything else is refused.
pub fn validate_entry_id(id: &str) -> Result<(String, String), String> {
    let refuse = || format!("{NAME}: '{}' is not a 1Password entry id", sanitize_display(id.trim(), 60));
    let (vault, item) = id.trim().split_once('/').ok_or_else(refuse)?;
    if !is_op_id(vault) || !is_op_id(item) {
        return Err(refuse());
    }
    Ok((vault.to_string(), item.to_string()))
}

/// The command for one call, inheriting from `parent_env` only what the
/// allowlist lets through. Pure apart from building it, so tests can look
/// at its arguments and environment.
pub fn build_command_with_env(
    bin: &Path,
    cfg: &OnePasswordConfig,
    secrets: &OnePasswordSecrets,
    op: &Op,
    parent_env: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<tokio::process::Command, String> {
    #[cfg(desktop)]
    let mut cmd = super::process::silent_command(bin);
    #[cfg(not(desktop))]
    let mut cmd = tokio::process::Command::new(bin);

    cli::set_base_env(&SPEC, &mut cmd, parent_env);
    cmd.args(op.args());
    if *op == Op::Version {
        return Ok(cmd);
    }
    cmd.arg("--no-color");
    match cfg.auth {
        OnePasswordAuth::App => {
            let account = cfg.account.trim();
            if !account.is_empty() {
                // Checked when saved: no spaces, never starts with '-'.
                cmd.arg(format!("--account={account}"));
            }
        }
        OnePasswordAuth::ServiceAccount => {
            let token = secrets.service_account_token.as_ref().ok_or_else(|| {
                format!("{NAME}: no service account token is stored. Add it in Settings → Password managers.")
            })?;
            cmd.env("OP_SERVICE_ACCOUNT_TOKEN", token.as_str());
        }
    }
    Ok(cmd)
}

// ---------------------------------------------------------------------------
// Output

#[derive(Deserialize)]
struct RawVault {
    #[serde(default)]
    id: Option<String>,
}

#[derive(Deserialize)]
struct RawUrl {
    #[serde(default)]
    href: Option<String>,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    vault: Option<RawVault>,
    /// For a login, its username.
    #[serde(default)]
    additional_information: Option<String>,
    #[serde(default)]
    urls: Vec<RawUrl>,
}

fn clean_opt(s: Option<String>, max: usize) -> Option<String> {
    s.map(|v| sanitize_display(v.trim(), max)).filter(|v| !v.is_empty())
}

/// The entries in `op item list --format json` output that hold a
/// password. Items without valid ids are skipped.
pub fn parse_list(stdout: &[u8]) -> Result<Vec<EntrySummary>, String> {
    let raw: Vec<RawItem> = serde_json::from_slice(stdout)
        .map_err(|_| format!("{NAME}: the CLI's list output was not understood (is 1Password CLI v2 installed?)"))?;
    let mut out: Vec<EntrySummary> = raw
        .into_iter()
        .filter_map(|e| {
            let category = e.category.as_deref().unwrap_or("").to_ascii_uppercase();
            if !PASSWORD_CATEGORIES.contains(&category.as_str()) {
                return None;
            }
            let vault = e.vault.and_then(|v| v.id).filter(|v| is_op_id(v))?;
            let item = e.id.filter(|i| is_op_id(i))?;
            let uri = e
                .urls
                .iter()
                .find(|u| u.primary)
                .or_else(|| e.urls.first())
                .and_then(|u| clean_opt(u.href.clone(), 1024));
            Some(EntrySummary {
                id: format!("{vault}/{item}"),
                name: clean_opt(e.title, 256).unwrap_or_default(),
                username: if category == "LOGIN" { clean_opt(e.additional_information, 256) } else { None },
                uri,
                folder_id: Some(vault),
            })
        })
        .collect();
    out.sort_by_cached_key(|e| e.name.to_lowercase());
    Ok(out)
}

/// The secret `op read --no-newline` printed.
pub fn parse_read(stdout: &[u8]) -> Result<Zeroizing<String>, String> {
    let text = std::str::from_utf8(stdout).map_err(|_| format!("{NAME}: the entry's password is not text"))?;
    // `--no-newline` should leave none; an older CLI may print one.
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
    let value = Zeroizing::new(trimmed.to_string());
    if value.is_empty() {
        return Err(format!("{NAME}: the entry has no password"));
    }
    Ok(value)
}

/// The version from `op --version`: `2.30.3`.
pub fn parse_version(stdout: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stdout);
    text.split_whitespace().next().map(|v| sanitize_display(v, 40)).filter(|v| !v.is_empty())
}

/// A user-facing message for a failed call, from its stderr: the CLI's last
/// line without its `[ERROR] date time` prefix, or a plain explanation of
/// the usual failures.
pub fn friendly_error(stderr: &[u8]) -> String {
    let text = cli::strip_control(&String::from_utf8_lossy(stderr));
    let last = text.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("");
    let detail = strip_log_prefix(last);
    let lower = text.to_lowercase();
    let has = |needle: &str| lower.contains(needle);
    let friendly = if has("authorization prompt dismissed") || has("authorization timeout") {
        "the request was not approved in the 1Password app.".to_string()
    } else if has("connecting to desktop app") || has("cannot connect to 1password app") || has("1password app is not running")
    {
        "could not reach the 1Password app. Start it and turn on Settings → Developer → Integrate with 1Password CLI.".to_string()
    } else if has("no accounts configured") || has("account is not signed in") || has("not currently signed in") {
        "not signed in. Turn on Settings → Developer → Integrate with 1Password CLI in the 1Password app, or use a service account.".to_string()
    } else if has("multiple accounts") || has("more than one account") {
        "the 1Password app is signed in to several accounts. Set the account to use in Settings → Password managers.".to_string()
    } else if has("isn't an item") || has("isn't a vault") || has("could not find item") || has("not found") {
        "entry not found. It may have been deleted or moved, or you no longer have access to it.".to_string()
    } else if has("does not have a field") || has("isn't a field") {
        "the entry has no password field.".to_string()
    } else if has("invalid service account token") || has("unauthorized") || has("(401)") {
        "the service account token was not accepted.".to_string()
    } else if detail.is_empty() {
        "the CLI failed without saying why.".to_string()
    } else {
        cli::truncate_chars(detail, MAX_ERROR_CHARS)
    };
    format!("{NAME}: {friendly}")
}

/// `[ERROR] 2026/11/15 10:11:12 message` → `message`.
fn strip_log_prefix(line: &str) -> &str {
    let Some(rest) = line.strip_prefix('[').and_then(|r| r.split_once("] ")).map(|(_, r)| r) else {
        return line;
    };
    let mut parts = rest.splitn(3, ' ');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(date), Some(time), Some(msg)) if date.contains('/') && time.contains(':') => msg,
        _ => rest,
    }
}

// ---------------------------------------------------------------------------
// The provider

/// 1Password, with the configuration and secret for one call.
pub(crate) struct OnePassword {
    cfg: OnePasswordConfig,
    secrets: OnePasswordSecrets,
    master_enabled: bool,
}

impl OnePassword {
    pub(crate) fn new(cfg: OnePasswordConfig, secrets: OnePasswordSecrets, master_enabled: bool) -> Self {
        Self { cfg, secrets, master_enabled }
    }

    /// Switched on, and with a service account, its token stored.
    fn configured(&self) -> bool {
        self.cfg.enabled
            && match self.cfg.auth {
                OnePasswordAuth::App => true,
                OnePasswordAuth::ServiceAccount => self.secrets.service_account_token.is_some(),
            }
    }

    #[cfg(desktop)]
    async fn run(&self, op: &Op) -> Result<Zeroizing<Vec<u8>>, Failure> {
        use super::process::{run, RunError};

        if *op != Op::Version && !self.cfg.enabled {
            return Err(Failure::Other(format!("{NAME} is turned off. Turn it on in Settings → Password managers.")));
        }
        let bin = cli::resolve_binary(&SPEC, self.cfg.cli_path.as_deref())?;
        let timeout = match op {
            Op::Version => VERSION_TIMEOUT,
            // Includes the time the user takes to approve in the app.
            _ => Duration::from_secs(u64::from(self.cfg.timeout_secs())),
        };
        let slot = async {
            match op {
                Op::Version => Ok(None),
                _ => super::process::cli_slot().await.map(Some).map_err(|e| format!("{NAME}: {e}")),
            }
        };
        let (_slot, cmd) = super::passbolt::build_when_ready(slot, || {
            build_command_with_env(&bin, &self.cfg, &self.secrets, op, std::env::vars_os())
        })
        .await?;
        let out = run(cmd, timeout, STDOUT_CAP, STDERR_CAP).await.map_err(|e| match e {
            RunError::Spawn(e) if e.kind() == std::io::ErrorKind::NotFound => {
                format!("{NAME}: the op CLI was not found. {INSTALL_HINT}")
            }
            RunError::Spawn(e) => format!("{NAME}: could not start the CLI: {}", e.kind()),
            RunError::Timeout => format!("{NAME}: no answer within {} seconds.", timeout.as_secs()),
            RunError::TooLarge => format!("{NAME}: the CLI printed more than Reach accepts."),
            RunError::Io(e) => format!("{NAME}: reading the CLI's output failed: {}", e.kind()),
        })?;
        if !out.status.success() {
            return Err(Failure::Other(friendly_error(&out.stderr)));
        }
        Ok(out.stdout)
    }

    #[cfg(not(desktop))]
    async fn run(&self, _op: &Op) -> Result<Zeroizing<Vec<u8>>, Failure> {
        Err(Failure::Other(format!("{NAME}: available on desktop only")))
    }
}

impl PasswordManager for OnePassword {
    async fn status(&self) -> ProviderStatus {
        let mut status = ProviderStatus {
            id: ProviderId::OnePassword.as_str().into(),
            name: NAME.into(),
            enabled: self.master_enabled && self.cfg.enabled,
            available: false,
            configured: self.configured(),
            version: None,
            detail: None,
        };
        if !cfg!(desktop) {
            status.detail = Some("Available on desktop only".into());
            return status;
        }
        match cli::resolve_binary(&SPEC, self.cfg.cli_path.as_deref()) {
            Ok(bin) => {
                status.available = true;
                status.detail = Some(bin.display().to_string());
                if status.enabled {
                    if let Ok(out) = self.run(&Op::Version).await {
                        status.version = parse_version(&out);
                    }
                }
            }
            Err(e) => status.detail = Some(e),
        }
        status
    }

    /// Nothing to type in Reach: the 1Password app asks, or the token is
    /// stored.
    fn passphrase_prompt(&self) -> Option<u32> {
        None
    }

    fn server_host(&self) -> Option<String> {
        None
    }

    fn hash_account(&self, mut h: &mut dyn Hasher) {
        let c = &self.cfg;
        c.cli_path.as_deref().map(str::trim).hash(&mut h);
        c.auth.hash(&mut h);
        c.account.trim().hash(&mut h);
        self.secrets.service_account_token.as_ref().map(|t| t.as_bytes()).hash(&mut h);
    }

    /// The password only, for now.
    fn check_entry(&self, entry_id: &str, field: Option<&str>) -> Result<String, String> {
        if let Some(f) = field {
            return Err(super::unsupported_field(ProviderId::OnePassword, f));
        }
        validate_entry_id(entry_id).map(|(v, i)| format!("{v}/{i}"))
    }

    async fn list(&self, _passphrase: Option<&Passphrase>) -> Result<Vec<EntrySummary>, Failure> {
        let out = self.run(&Op::List).await?;
        Ok(parse_list(&out)?)
    }

    async fn resolve(&self, entry_id: &str, _passphrase: Option<&Passphrase>) -> Result<Zeroizing<String>, Failure> {
        let (vault, item) = validate_entry_id(entry_id)?;
        let out = self.run(&Op::Read { vault, item }).await?;
        Ok(parse_read(&out)?)
    }
}

#[cfg(test)]
#[path = "onepassword_tests.rs"]
mod tests;
