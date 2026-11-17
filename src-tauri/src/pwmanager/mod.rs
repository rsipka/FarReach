//! External password managers.
//!
//! A saved session, or a database connection, can point a secret field at an
//! entry in a password manager instead of keeping the secret in Reach's own
//! vault. What is stored is a [`SecretRef`]: which manager, which entry, and a
//! label for the editor to show. The secret itself is fetched here, in the
//! backend, at the moment a connection needs it, handed to the code that logs
//! in, and never sent to the webview. Nothing fetched is cached: the next
//! connect asks the manager again.
//!
//! Off by default (Settings → Password managers). Every command that talks to
//! a manager checks [`require`] first, so "off" means nothing runs, not just
//! that the settings page is hidden.
//!
//! Each manager is one [`PasswordManager`] implementation and one
//! [`ProviderId`] arm; dispatch is a `match`, not a trait object. Passbolt,
//! 1Password and Bitwarden are driven through their official command-line
//! clients (see [`passbolt`], [`onepassword`], [`bitwarden`]): desktop only,
//! since on Android and iOS there is no CLI to run, and each says so in its
//! status. KeePass databases are read in process (see [`keepass`]).
//!
//! The manager's own login may need a passphrase the user chose not to
//! store. Then it is asked for in the window when a call needs it, and kept
//! encrypted in memory for as long as the user said, or only for the one
//! operation (a connect, a listing) that asked: see [`unlock`]. Every call
//! that can ask runs inside an operation ([`with_prompt`]), which carries the
//! window to ask in and what to say the passphrase is for.
//!
//! # Adding a manager
//!
//! Every manager goes through the same rules: nothing secret reaches the
//! webview, every reference is approved by the person connecting, listings
//! hold metadata only, and a CLI is run the way [`cli`] says. A new one is:
//!
//! 1. **A file** (`bitwarden.rs`, say) with a struct built from the loaded
//!    configuration and a [`PasswordManager`] implementation: `status`,
//!    `list`, `resolve`, `check_entry` (entry id and field), `hash_account`
//!    (what decides whose entries are seen), and `passphrase_prompt` /
//!    `secret_kind` if the user types something to unlock it. A tool run
//!    as a child process describes itself in a [`cli::CliSpec`] and uses
//!    [`cli`] for the binary and its environment, `process::run` to run it
//!    (one slot, a deadline, capped output).
//! 2. **An arm** in [`ProviderId`] (`ALL`, `as_str`, `display_name`,
//!    `parse`) and in each `match` of [`Provider`].
//! 3. **Its settings** in `config`: a section of [`PasswordManagersConfig`]
//!    with its own `enabled` switch (synced, non-secret), its secrets as
//!    separate `pwmanager.<id>.*` settings, and anything that names a file
//!    on this computer (a CLI path, a database file) in
//!    [`config::DeviceSettings`], never in the synced vault. Its view and
//!    input go in `PasswordManagersView` / `PasswordManagersConfigInput`,
//!    with secrets write-only.
//! 4. **A settings card** in the frontend
//!    (`src/lib/components/settings/`), shown by `PasswordManagersTab`, and
//!    its strings in every locale (`npm run i18n:check`).
//! 5. **Tests** with a fake CLI (see `passbolt_tests.rs`): the command
//!    line holds no secret, the environment is the allowlist, output is
//!    parsed into metadata only, errors are cleaned.
//! 6. **User docs**: the Password managers entry in `CHANGELOG.md`.

pub mod bitwarden;
mod cache;
pub(crate) mod cli;
pub mod config;
pub mod keepass;
pub mod onepassword;
pub mod passbolt;
#[cfg(desktop)]
mod process;
pub(crate) mod prompt;
pub mod unlock;
#[cfg(windows)]
mod winacl;

use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::state::AppState;
use config::PasswordManagersConfig;

/// A pointer to a secret held by a password manager. Holds no secret itself,
/// so it may be logged, shown and shared with the session.
///
/// A session can sit in a vault other people write to, and a reference is
/// resolved with the connecting person's own manager login. So a reference
/// only counts once that person has said this session may send this entry
/// to where it connects: each such word is a signature in `approvals`, made
/// the way ssh_config approvals are (see `ssh::sshconf::approvals` and
/// `ipc::session_commands::ref_slots`). `trusted` says whether the person
/// reading the session gave one that still holds; it is worked out on every
/// read and never taken from what is sent in.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRef {
    /// Which manager: `"passbolt"`, `"onepassword"`, `"bitwarden"`, `"keepass"`.
    pub provider: String,
    /// The manager's own id for the entry (Passbolt: the resource UUID).
    pub entry_id: String,
    /// Which of the entry's fields holds the secret. `None` is the
    /// password, and is what every reference saved before this existed
    /// means. Each manager says which fields it can read
    /// ([`PasswordManager::check_entry`]). Part of what is approved: a
    /// reference to another field of the same entry needs approving again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// The entry's name when it was picked. For display only: never used to
    /// find the entry, and never trusted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Signed approvals, one per person who allowed this entry for this
    /// session as it is. Stored with the session.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<String>,
    /// The reading person has approved this reference for the session as it
    /// now is. Sent to the editor and the session list; never read from
    /// input, never stored.
    #[serde(default, skip_deserializing, skip_serializing_if = "is_false")]
    pub trusted: bool,
    /// The editor's word that the person picked (or confirmed) this reference
    /// in this edit, so saving signs it. Read from input only, never sent
    /// back and never stored.
    #[serde(default, skip_serializing)]
    pub confirm: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl SecretRef {
    /// The same field of the same entry of the same manager; the label and
    /// the approvals aside.
    pub fn same_entry(&self, other: &SecretRef) -> bool {
        self.provider == other.provider && self.entry_id == other.entry_id && self.field() == other.field()
    }

    /// The field, with the password (the default) as `None` however it was
    /// written.
    pub fn field(&self) -> Option<&str> {
        self.field.as_deref().map(str::trim).filter(|f| !f.is_empty() && *f != FIELD_PASSWORD)
    }
}

/// The field every manager can read, and the one a reference without a
/// field means.
pub const FIELD_PASSWORD: &str = "password";

/// The fields a manager does not read are refused with this.
pub(crate) fn unsupported_field(id: ProviderId, field: &str) -> String {
    format!("{}: Reach cannot read the field '{}' of an entry", id.display_name(), sanitize_display(field, 40))
}

/// One entry as the picker lists it: metadata only, never a secret.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EntrySummary {
    pub id: String,
    pub name: String,
    pub username: Option<String>,
    pub uri: Option<String>,
    pub folder_id: Option<String>,
}

/// What Settings shows for one manager. Finding the manager's tool is all
/// this does; it never logs in.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    /// The master switch (Settings → Password managers).
    pub enabled: bool,
    /// The tool this manager needs is there (and this platform can run it).
    pub available: bool,
    /// Enough is set up to try a login.
    pub configured: bool,
    pub version: Option<String>,
    /// Where the tool was found, or why it was not.
    pub detail: Option<String>,
}

/// The outcome of "Test connection": a real login and a metadata listing.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TestResult {
    pub ok: bool,
    pub entries: u32,
    pub message: String,
}

/// Every manager Reach knows. Adding one is a new file with a
/// [`PasswordManager`] implementation and an arm here and in [`Provider`]:
/// see docs/password-managers.md for the whole list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId {
    Passbolt,
    OnePassword,
    Bitwarden,
    Keepass,
}

impl ProviderId {
    pub const ALL: &'static [ProviderId] =
        &[ProviderId::Passbolt, ProviderId::OnePassword, ProviderId::Bitwarden, ProviderId::Keepass];

    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Passbolt => "passbolt",
            ProviderId::OnePassword => "onepassword",
            ProviderId::Bitwarden => "bitwarden",
            ProviderId::Keepass => "keepass",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ProviderId::Passbolt => "Passbolt",
            ProviderId::OnePassword => "1Password",
            ProviderId::Bitwarden => "Bitwarden",
            ProviderId::Keepass => "KeePass",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "passbolt" => Ok(ProviderId::Passbolt),
            "onepassword" => Ok(ProviderId::OnePassword),
            "bitwarden" => Ok(ProviderId::Bitwarden),
            "keepass" => Ok(ProviderId::Keepass),
            other => Err(format!("Unknown password manager '{}'", sanitize_display(other, 40))),
        }
    }
}

/// Why a call to a manager failed: the passphrase that unlocks the login was
/// wrong (worth asking again), or anything else. Both carry the user-facing
/// message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    WrongPassphrase(String),
    Other(String),
}

impl Failure {
    pub fn into_message(self) -> String {
        match self {
            Failure::WrongPassphrase(m) | Failure::Other(m) => m,
        }
    }
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::Other(message)
    }
}

/// One password manager. Each call is independent: nothing here keeps a
/// login open between calls.
pub(crate) trait PasswordManager {
    /// Find the tool and its version. No login.
    async fn status(&self) -> ProviderStatus;
    /// `Some(seconds to keep it)` when a login needs a passphrase the user
    /// types (see [`unlock`]); `None` when the configuration brings its own.
    fn passphrase_prompt(&self) -> Option<u32>;
    /// Every entry the user can see, metadata only. `passphrase` is the
    /// typed one, when [`passphrase_prompt`](Self::passphrase_prompt) asked
    /// for it.
    async fn list(&self, passphrase: Option<&unlock::Passphrase>) -> Result<Vec<EntrySummary>, Failure>;
    /// The password of one entry.
    async fn resolve(&self, entry_id: &str, passphrase: Option<&unlock::Passphrase>) -> Result<Zeroizing<String>, Failure>;
    /// The host name of the server the login goes to, if known: shown in
    /// the passphrase question so two accounts can be told apart. Not
    /// secret.
    fn server_host(&self) -> Option<String>;
    /// What the typed secret is called in the question to the user.
    fn secret_kind(&self) -> SecretKind {
        SecretKind::Passphrase
    }
    /// Feed `h` everything that decides whose entries a call sees (the
    /// tool, the server, the account, a key): listings and a typed
    /// passphrase are filed under it (see [`cache::CacheKey`]), so other
    /// settings never get them. Hashed with a per-run random key, never
    /// stored or logged.
    fn hash_account(&self, h: &mut dyn std::hash::Hasher);
    /// Check a reference's entry id and field before anything is run with
    /// them; `Ok` with the id as the manager wants it.
    fn check_entry(&self, entry_id: &str, field: Option<&str>) -> Result<String, String>;
}

/// What the secret a manager asks for is called, for the question's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKind {
    /// Passbolt: the passphrase of the user's private key.
    Passphrase,
    /// The master password of a vault (Bitwarden, KeePass).
    MasterPassword,
}

/// A manager, configured and ready to call. Built fresh for every call from
/// what the settings vault holds, and dropped (with the secrets it carries)
/// right after.
pub(crate) enum Provider {
    Passbolt(passbolt::Passbolt),
    OnePassword(onepassword::OnePassword),
    Bitwarden(bitwarden::Bitwarden),
    Keepass(keepass::Keepass),
}

impl Provider {
    fn new(id: ProviderId, loaded: config::Loaded) -> Self {
        match id {
            ProviderId::Passbolt => Provider::Passbolt(passbolt::Passbolt::new(
                loaded.config.passbolt,
                loaded.passbolt_secrets,
                loaded.config.enabled,
            )),
            ProviderId::OnePassword => Provider::OnePassword(onepassword::OnePassword::new(
                loaded.config.onepassword,
                loaded.onepassword_secrets,
                loaded.config.enabled,
            )),
            ProviderId::Bitwarden => {
                Provider::Bitwarden(bitwarden::Bitwarden::new(loaded.config.bitwarden, loaded.config.enabled))
            }
            ProviderId::Keepass => Provider::Keepass(keepass::Keepass::new(loaded.config.keepass, loaded.config.enabled)),
        }
    }

    async fn status(&self) -> ProviderStatus {
        match self {
            Provider::Passbolt(p) => p.status().await,
            Provider::OnePassword(p) => p.status().await,
            Provider::Bitwarden(p) => p.status().await,
            Provider::Keepass(p) => p.status().await,
        }
    }

    fn passphrase_prompt(&self) -> Option<u32> {
        match self {
            Provider::Passbolt(p) => p.passphrase_prompt(),
            Provider::OnePassword(p) => p.passphrase_prompt(),
            Provider::Bitwarden(p) => p.passphrase_prompt(),
            Provider::Keepass(p) => p.passphrase_prompt(),
        }
    }

    async fn list(&self, passphrase: Option<&unlock::Passphrase>) -> Result<Vec<EntrySummary>, Failure> {
        match self {
            Provider::Passbolt(p) => p.list(passphrase).await,
            Provider::OnePassword(p) => p.list(passphrase).await,
            Provider::Bitwarden(p) => p.list(passphrase).await,
            Provider::Keepass(p) => p.list(passphrase).await,
        }
    }

    async fn resolve(&self, entry_id: &str, passphrase: Option<&unlock::Passphrase>) -> Result<Zeroizing<String>, Failure> {
        match self {
            Provider::Passbolt(p) => p.resolve(entry_id, passphrase).await,
            Provider::OnePassword(p) => p.resolve(entry_id, passphrase).await,
            Provider::Bitwarden(p) => p.resolve(entry_id, passphrase).await,
            Provider::Keepass(p) => p.resolve(entry_id, passphrase).await,
        }
    }

    fn server_host(&self) -> Option<String> {
        match self {
            Provider::Passbolt(p) => p.server_host(),
            Provider::OnePassword(p) => p.server_host(),
            Provider::Bitwarden(p) => p.server_host(),
            Provider::Keepass(p) => p.server_host(),
        }
    }

    fn id(&self) -> ProviderId {
        match self {
            Provider::Passbolt(_) => ProviderId::Passbolt,
            Provider::OnePassword(_) => ProviderId::OnePassword,
            Provider::Bitwarden(_) => ProviderId::Bitwarden,
            Provider::Keepass(_) => ProviderId::Keepass,
        }
    }

    fn secret_kind(&self) -> SecretKind {
        match self {
            Provider::Passbolt(p) => p.secret_kind(),
            Provider::OnePassword(p) => p.secret_kind(),
            Provider::Bitwarden(p) => p.secret_kind(),
            Provider::Keepass(p) => p.secret_kind(),
        }
    }

    pub(crate) fn hash_account(&self, mut h: &mut dyn std::hash::Hasher) {
        self.id().as_str().hash(&mut h);
        match self {
            Provider::Passbolt(p) => p.hash_account(h),
            Provider::OnePassword(p) => p.hash_account(h),
            Provider::Bitwarden(p) => p.hash_account(h),
            Provider::Keepass(p) => p.hash_account(h),
        }
    }

    fn check_entry(&self, entry_id: &str, field: Option<&str>) -> Result<String, String> {
        match self {
            Provider::Passbolt(p) => p.check_entry(entry_id, field),
            Provider::OnePassword(p) => p.check_entry(entry_id, field),
            Provider::Bitwarden(p) => p.check_entry(entry_id, field),
            Provider::Keepass(p) => p.check_entry(entry_id, field),
        }
    }

    /// What a call must unlock, if the user is to be asked.
    fn unlock_policy(&self, account: cache::CacheKey) -> Option<unlock::Policy> {
        self.passphrase_prompt().map(|remember_secs| unlock::Policy {
            account,
            remember_secs,
            server: self.server_host(),
            secret_kind: self.secret_kind(),
        })
    }

    /// [`list`](Self::list), asking for the passphrase if it must be.
    async fn list_unlocked(&self, account: cache::CacheKey) -> Result<Vec<EntrySummary>, String> {
        unlock::global()
            .with_passphrase(self.unlock_policy(account), |p| async move { self.list(p.as_deref()).await })
            .await
    }

    /// [`resolve`](Self::resolve), asking for the passphrase if it must be.
    async fn resolve_unlocked(&self, account: cache::CacheKey, entry_id: &str) -> Result<Zeroizing<String>, String> {
        unlock::global()
            .with_passphrase(self.unlock_policy(account), |p| async move { self.resolve(entry_id, p.as_deref()).await })
            .await
    }
}

// ---------------------------------------------------------------------------
// Asking for the passphrase.

pub use unlock::Reason;

/// Run `fut` as one operation that may ask the user for a manager's
/// passphrase in `app`'s window, saying it is for `reason`. Whatever `fut`
/// resolves, the passphrase is asked for at most once (see [`unlock`]).
///
/// `fut` is boxed: it is often a whole SSH login, and inline it is part of
/// every future around it. A database connect through a session nests two
/// of them, which overflowed a worker thread's stack in a debug build.
pub async fn with_prompt<F: Future>(app: &tauri::AppHandle, reason: Reason, fut: F) -> F::Output {
    unlock::operation(Some(Arc::new(app.clone())), reason, Box::pin(fut)).await
}

/// Name what the current operation connects to, once known.
pub fn name_operation(name: &str) {
    unlock::name_operation(name);
}

/// Forget every passphrase kept in memory, and cancel every passphrase
/// question still open (an answer to it would not be kept anyway): "Forget
/// now", a settings save, a vault lock or a new identity, the computer
/// locking or going to sleep, quitting.
pub fn forget_passphrase() {
    unlock::global().cache.forget();
    bitwarden::sessions::forget();
    prompt::cancel_all();
}

/// The vault was locked, or its identity replaced (a new one, an import, a
/// reset): the cached entry lists and any passphrase kept in memory belong
/// to what is gone. Both go, and any open passphrase question is cancelled.
pub fn forget_all() {
    clear_cache();
    forget_passphrase();
    // Key passphrases kept for this run go with the vault as well.
    crate::ssh::unlocked::forget();
}

/// Whether a typed passphrase is kept in memory now for the account these
/// settings make `id` log in to.
pub fn passphrase_cached(id: ProviderId, loaded: &config::Loaded) -> bool {
    Provider::new(id, loaded.clone()).passphrase_prompt().is_some()
        && unlock::global().cache.holds(cache::CacheKey::new(id, loaded))
}

/// The window's answer to a passphrase question (`None`: cancelled).
pub fn answer_prompt(prompt_id: &str, passphrase: Option<String>) -> Result<(), String> {
    prompt::answer(prompt_id, passphrase)
}

/// The message every gated call returns while the master switch is off.
pub const DISABLED_MESSAGE: &str =
    "Password managers are turned off. Turn them on in Settings → Password managers.";

/// Refuse unless the user turned password managers on.
pub fn require(config: &PasswordManagersConfig) -> Result<(), String> {
    if config.enabled {
        Ok(())
    } else {
        Err(DISABLED_MESSAGE.to_string())
    }
}

// ---------------------------------------------------------------------------
// Entry listings. Metadata only (names, usernames, URLs), cached for a few
// minutes per account, one login at a time per provider: see [`cache`].

/// The most entries the picker is sent at once; the search box narrows it.
pub const MAX_LIST_RESULTS: usize = 500;

/// Forget every cached listing, and keep any listing still running from
/// being cached: the configuration changed, the feature was switched off,
/// or the vault was locked.
pub fn clear_cache() {
    cache::global().clear();
}

/// Case-insensitive substring match on name, username and URL, at most
/// [`MAX_LIST_RESULTS`] entries.
pub fn filter_entries(entries: &[EntrySummary], query: Option<&str>) -> Vec<EntrySummary> {
    let needle = query.map(str::trim).filter(|q| !q.is_empty()).map(str::to_lowercase);
    let matches = |e: &&EntrySummary| match &needle {
        None => true,
        Some(n) => {
            e.name.to_lowercase().contains(n)
                || e.username.as_deref().is_some_and(|u| u.to_lowercase().contains(n))
                || e.uri.as_deref().is_some_and(|u| u.to_lowercase().contains(n))
        }
    };
    entries.iter().filter(matches).take(MAX_LIST_RESULTS).cloned().collect()
}

/// Text from outside (an entry name, an id the frontend sent) made safe to
/// show: control characters dropped, length capped.
pub(crate) fn sanitize_display(s: &str, max_chars: usize) -> String {
    let cleaned: String = s.chars().filter(|c| !c.is_control()).collect();
    if cleaned.chars().count() > max_chars {
        let mut t: String = cleaned.chars().take(max_chars).collect();
        t.push('…');
        t
    } else {
        cleaned
    }
}

// ---------------------------------------------------------------------------
// The calls the rest of the app makes.

async fn provider_for(state: &AppState, id: ProviderId) -> Result<(Provider, cache::CacheKey), String> {
    let loaded = config::load(state).await?;
    require(&loaded.config)?;
    let key = cache::CacheKey::new(id, &loaded);
    Ok((Provider::new(id, loaded), key))
}

/// Every manager's status, for Settings. Allowed while switched off: it only
/// looks for the tool (and asks its version once switched on), it never logs
/// in.
pub async fn status_all(state: &AppState) -> Vec<ProviderStatus> {
    let loaded = config::load(state).await.unwrap_or_default();
    let mut out = Vec::with_capacity(ProviderId::ALL.len());
    for id in ProviderId::ALL {
        out.push(Provider::new(*id, loaded.clone()).status().await);
    }
    out
}

/// The entries of one manager, from the cache unless `refresh`. Callers that
/// miss the cache at the same time share one login.
pub async fn list(
    state: &AppState,
    id: ProviderId,
    query: Option<&str>,
    refresh: bool,
) -> Result<Vec<EntrySummary>, String> {
    let cache = cache::global();
    // Before the settings are read: a save from here on makes this listing
    // one that is not kept.
    let generation = cache.generation();
    let loaded = config::load(state).await?;
    require(&loaded.config)?;
    let key = cache::CacheKey::new(id, &loaded);
    let provider = Provider::new(id, loaded);
    let all = cache.get_or_fetch(key, generation, refresh, || provider.list_unlocked(key)).await?;
    Ok(filter_entries(&all, query))
}

/// Log in and list, as "Test connection" does. Failures are a result, not
/// an error, so Settings can show them in place.
pub async fn test(state: &AppState, id: ProviderId) -> Result<TestResult, String> {
    let cache = cache::global();
    let generation = cache.generation();
    let loaded = config::load(state).await?;
    require(&loaded.config)?;
    let key = cache::CacheKey::new(id, &loaded);
    let provider = Provider::new(id, loaded);
    let started = Instant::now();
    match provider.list_unlocked(key).await {
        Ok(entries) => {
            let count = u32::try_from(entries.len()).unwrap_or(u32::MAX);
            cache.store(key, generation, Arc::new(entries));
            tracing::info!(
                "pwmanager: test provider={} ok entries={count} in {}ms",
                id.as_str(),
                started.elapsed().as_millis()
            );
            Ok(TestResult {
                ok: true,
                entries: count,
                message: format!("Connected to {}: {count} entries visible", id.display_name()),
            })
        }
        Err(message) => {
            // The message is the user-facing one: the CLI's last error line,
            // cleaned and cut short, never its output or environment.
            tracing::info!(
                "pwmanager: test provider={} failed in {}ms: {message}",
                id.as_str(),
                started.elapsed().as_millis()
            );
            Ok(TestResult { ok: false, entries: 0, message })
        }
    }
}

/// Refuse a reference the connecting person has not approved. `trusted` is
/// never read from input: it is set only when a stored session is read and
/// the person's own signature for it holds (see [`SecretRef`]), so a
/// reference that came any other way is refused here too.
pub fn check_trusted(r: &SecretRef) -> Result<(), String> {
    if r.trusted {
        return Ok(());
    }
    let name = ProviderId::parse(&r.provider).map(ProviderId::display_name).unwrap_or("Password manager");
    Err(format!(
        "{name}: this session's password manager entry is not confirmed. Connect to the session from the session list to confirm it."
    ))
}

/// The secret a session's reference points at, fetched now, if the
/// connecting person approved it ([`check_trusted`]). Errors name the
/// manager and never contain a secret.
pub async fn resolve(state: &AppState, r: &SecretRef) -> Result<Zeroizing<String>, String> {
    check_trusted(r)?;
    fetch(state, r).await
}

/// [`resolve`] without the approval check, for a reference nobody but the
/// user can have written: a database connection's own password. Those are
/// kept in the `__databases__` vault, which is private (its key is wrapped
/// for this identity alone; with personal sync it goes to this identity's
/// own devices, never to a shared vault, and a full backup restore replaces
/// the identity with the one that made it), or come from the form the user
/// is filling in. The SSH leg of such a connection is a saved session and
/// goes through [`resolve`].
pub async fn resolve_own(state: &AppState, r: &SecretRef) -> Result<Zeroizing<String>, String> {
    fetch(state, r).await
}

/// [`resolve_own`], into a plain `String`.
pub async fn resolve_own_to_string(state: &AppState, r: &SecretRef) -> Result<String, String> {
    resolve_own(state, r).await.map(|s| s.as_str().to_owned())
}

async fn fetch(state: &AppState, r: &SecretRef) -> Result<Zeroizing<String>, String> {
    let id = ProviderId::parse(&r.provider)?;
    let (provider, key) = provider_for(state, id).await.map_err(|e| format!("{}: {e}", id.display_name()))?;
    let entry_id = provider.check_entry(&r.entry_id, r.field())?;
    let started = Instant::now();
    let result = provider.resolve_unlocked(key, &entry_id).await;
    tracing::info!(
        "pwmanager: resolve provider={} entry={} ok={} in {}ms",
        id.as_str(),
        sanitize_display(&r.entry_id, 64),
        result.is_ok(),
        started.elapsed().as_millis()
    );
    result
}

/// [`resolve`], into the plain `String` the login code takes. For callers
/// that hand the secret straight on.
pub async fn resolve_to_string(state: &AppState, r: &SecretRef) -> Result<String, String> {
    resolve(state, r).await.map(|s| s.as_str().to_owned())
}

/// A saved session's login with its references filled in: a reference wins
/// over whatever plaintext the session also holds.
pub async fn fill_auth_method(
    state: &AppState,
    method: &crate::state::AuthMethod,
) -> Result<crate::state::AuthMethod, String> {
    use crate::state::AuthMethod;
    let mut method = method.clone();
    match &mut method {
        AuthMethod::Password { password, password_ref: Some(r) } => {
            *password = Some(resolve_to_string(state, r).await?);
        }
        AuthMethod::Key { passphrase, passphrase_ref: Some(r), .. } => {
            *passphrase = Some(resolve_to_string(state, r).await?);
        }
        _ => {}
    }
    Ok(method)
}

/// A proxy's password filled in from its reference, if it has one.
pub async fn fill_proxy(
    state: &AppState,
    proxy: &mut Option<crate::state::ProxyConfig>,
) -> Result<(), String> {
    if let Some(p) = proxy.as_mut() {
        if let Some(r) = p.password_ref.as_ref() {
            p.password = Some(resolve_to_string(state, r).await?);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
