//! What Settings → Password managers stores, and how.
//!
//! The non-secret part is one JSON value in the `__settings__` vault under
//! [`CONFIG_KEY`]. Each secret (the Passbolt private key, its passphrase, a
//! TOTP seed) is a separate entry in the same vault, encrypted like every
//! other setting, so the configuration can be read and shown without
//! touching them. No IPC command returns a secret: the view says only
//! whether one is stored (`has_private_key` and so on), and saving takes a
//! new value or leaves the stored one alone.
//!
//! The passphrase need not be stored at all ([`PassphraseMode::Ask`], the
//! default for a new setup): Reach then asks for it when a login needs it and
//! keeps it, encrypted, in memory only (see `super::unlock`). Switching from
//! stored to asked removes the stored passphrase, so the two never coexist.
//! Only the switch does, judged against the configuration stored at the
//! moment of the save (or an explicit `clear_passphrase`): a save that was
//! already asking leaves the vault's passphrase entry alone, so a form
//! filled from defaults cannot delete it by accident.
//!
//! One setting is not in the vault: the path of the CLI binary. The
//! `__settings__` vault syncs to every device of the account (and a shared
//! vault's members could reach it), and that binary is handed the private
//! key and passphrase, so a path checked on one computer must not become
//! the program another computer runs. It is kept per device, in
//! [`device_settings_path`]; a `cli_path` still found in the vault's JSON (from
//! an earlier build) is ignored and dropped at the next save.

use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::ipc::settings_commands::{delete_setting_by_key, read_setting_by_key, write_setting_by_key};
use crate::state::AppState;

/// The settings-vault key of the JSON configuration.
pub const CONFIG_KEY: &str = "password_managers";
/// The settings-vault keys of the Passbolt secrets.
pub const PASSBOLT_PRIVATE_KEY: &str = "pwmanager.passbolt.private_key";
pub const PASSBOLT_PASSPHRASE: &str = "pwmanager.passbolt.passphrase";
pub const PASSBOLT_TOTP_SECRET: &str = "pwmanager.passbolt.totp_secret";
/// The settings-vault key of the 1Password service account token.
pub const ONEPASSWORD_SERVICE_ACCOUNT_TOKEN: &str = "pwmanager.onepassword.service_account_token";
/// The longest 1Password account name (sign-in address, shorthand or ID).
const MAX_ACCOUNT_CHARS: usize = 256;
/// The longest service account token Reach takes. Tokens are about a
/// kilobyte; the cap keeps one inside a Windows environment variable.
pub const MAX_SERVICE_ACCOUNT_TOKEN_BYTES: usize = 16 * 1024;

/// Settings keys only this module may read or write. The generic settings
/// commands refuse them, so a secret cannot be read back through those.
pub fn is_reserved_setting_key(key: &str) -> bool {
    key == CONFIG_KEY || key.starts_with("pwmanager.")
}

/// The largest private key Reach takes, pasted or from a file. Armored
/// OpenPGP keys are a few kilobytes (RSA-4096: about 7 KB). The key goes to
/// the CLI in one environment variable, and Windows allows at most 32,767
/// characters in one, so the cap stays below that.
pub const MAX_PRIVATE_KEY_BYTES: u64 = 32_000;
const _: () = assert!(MAX_PRIVATE_KEY_BYTES < 32_767, "must fit in one Windows environment variable");
const PGP_PRIVATE_BEGIN: &str = "-----BEGIN PGP PRIVATE KEY BLOCK-----";
const PGP_PRIVATE_END: &str = "-----END PGP PRIVATE KEY BLOCK-----";
/// The longest passphrase Reach takes, in UTF-8 bytes, stored or typed when
/// asked. A passphrase that is asked for is held in a fixed-size shielded
/// buffer (see `super::unlock`), so it has to fit in one; a kilobyte is far
/// more than any passphrase that is typed.
pub const MAX_PASSPHRASE_BYTES: usize = 1024;
const MAX_SERVER_ADDRESS_CHARS: usize = 2048;
const MAX_CLI_PATH_CHARS: usize = 4096;

pub const DEFAULT_TIMEOUT_SECS: u32 = 60;
pub const MIN_TIMEOUT_SECS: u32 = 10;
pub const MAX_TIMEOUT_SECS: u32 = 300;

/// How long a passphrase that was asked for is kept in memory, by default:
/// fifteen minutes.
pub const DEFAULT_REMEMBER_SECS: u32 = 900;
/// The longest Reach keeps an asked passphrase: eight hours. Settings offers
/// up to four; a hand-edited value is cut to this.
pub const MAX_REMEMBER_SECS: u32 = 8 * 3600;

/// Where the Passbolt server address, key and passphrase come from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum CredentialsMode {
    /// Stored by Reach in its vault, handed to the CLI for each call. The
    /// user's own CLI configuration file is not read.
    #[default]
    Reach,
    /// The user's existing go-passbolt-cli configuration file, as the CLI
    /// would use it on its own.
    CliConfig,
}

/// How a Passbolt login answers a multi-factor challenge.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum MfaMode {
    /// No MFA on this account; a challenge fails the login.
    #[default]
    None,
    /// Codes generated from a stored TOTP seed.
    TotpSecret,
}

/// Where the passphrase that unlocks the Passbolt private key comes from,
/// with credentials stored by Reach.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum PassphraseMode {
    /// Asked for when a call needs it, and kept in memory (encrypted) for
    /// [`PassboltConfig::passphrase_remember_secs`] at most, or not at all.
    /// Never written anywhere. The default for a new setup.
    #[default]
    Ask,
    /// Stored in Reach's vault with the private key, and handed to the CLI
    /// for every call without asking.
    Stored,
}

/// The Passbolt settings that are not secret.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PassboltConfig {
    /// The `passbolt` binary. Empty: found on PATH. Never read from or
    /// written to the vault (see the module comment): [`load`] fills it in
    /// from this device's own settings file.
    #[serde(skip)]
    pub cli_path: Option<String>,
    pub credentials: CredentialsMode,
    pub server_address: String,
    pub mfa_mode: MfaMode,
    pub timeout_secs: u32,
    /// Asked for or stored. `None` only in a configuration saved before this
    /// setting existed: [`load`] decides it from whether a passphrase is
    /// stored (see [`PassboltConfig::settle_passphrase_mode`]). The field's
    /// own default (`None`), not the struct's, is what a missing key reads
    /// as, so the two cases can be told apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase_mode: Option<PassphraseMode>,
    /// How long an asked passphrase is kept in memory, in seconds. 0: not
    /// kept, asked again by the next connect (one connect that needs it
    /// several times, for jump hosts say, asks once).
    pub passphrase_remember_secs: u32,
}

impl Default for PassboltConfig {
    fn default() -> Self {
        Self {
            cli_path: None,
            credentials: CredentialsMode::Reach,
            server_address: String::new(),
            mfa_mode: MfaMode::None,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            passphrase_mode: Some(PassphraseMode::Ask),
            passphrase_remember_secs: DEFAULT_REMEMBER_SECS,
        }
    }
}

impl PassboltConfig {
    /// The timeout, kept inside the range Settings allows even if the stored
    /// value is not.
    pub fn timeout_secs(&self) -> u32 {
        clamp_timeout(self.timeout_secs)
    }

    /// Asked for or stored; asked for if not yet decided.
    pub fn passphrase_mode(&self) -> PassphraseMode {
        self.passphrase_mode.unwrap_or_default()
    }

    /// How long an asked passphrase is kept, within what Reach allows.
    pub fn remember_secs(&self) -> u32 {
        clamp_remember(self.passphrase_remember_secs)
    }

    /// Whether a login needs the user to type the passphrase: credentials
    /// stored by Reach, and the passphrase not among them. With the CLI's
    /// own configuration the passphrase is the CLI's business.
    pub fn asks_passphrase(&self) -> bool {
        self.credentials == CredentialsMode::Reach && self.passphrase_mode() == PassphraseMode::Ask
    }

    /// Decide the passphrase mode of a configuration saved before there was
    /// one: whoever stored a passphrase keeps using it, everyone else is
    /// asked. A mode already set is left alone.
    pub fn settle_passphrase_mode(&mut self, has_stored_passphrase: bool) {
        if self.passphrase_mode.is_none() {
            self.passphrase_mode =
                Some(if has_stored_passphrase { PassphraseMode::Stored } else { PassphraseMode::Ask });
        }
    }
}

/// How the 1Password CLI signs in.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnePasswordAuth {
    /// Through the 1Password desktop app ("Integrate with 1Password CLI"):
    /// the app asks the user to approve (Touch ID, Windows Hello, the system
    /// password or the account password). Reach stores and asks nothing.
    #[default]
    App,
    /// A service account token stored by Reach. Sees only the vaults the
    /// service account was given, never a Personal or Private vault.
    ServiceAccount,
}

/// The 1Password settings that are not secret.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct OnePasswordConfig {
    /// The `op` binary. Empty: found on PATH. Kept on this device only, as
    /// Passbolt's is.
    #[serde(skip)]
    pub cli_path: Option<String>,
    /// This manager's own switch, under the master one: off, nothing runs
    /// `op`.
    pub enabled: bool,
    pub auth: OnePasswordAuth,
    /// The account to use when the app is signed in to several: its sign-in
    /// address (`my.1password.com`), shorthand or ID. Empty: the CLI's
    /// default. Not used with a service account.
    pub account: String,
    pub timeout_secs: u32,
}

impl Default for OnePasswordConfig {
    fn default() -> Self {
        Self {
            cli_path: None,
            enabled: false,
            auth: OnePasswordAuth::App,
            account: String::new(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }
}

impl OnePasswordConfig {
    pub fn timeout_secs(&self) -> u32 {
        clamp_timeout(self.timeout_secs)
    }
}

/// The 1Password secret, read from the vault for one call and wiped when
/// dropped.
/// Settings → Password managers → Bitwarden. No secret is stored: the user
/// signs in with `bw login` themselves, and the master password is asked
/// for when a call needs it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct BitwardenConfig {
    /// Absolute path to `bw`. `None`: found on PATH. This device's own
    /// setting, never in the vault.
    #[serde(skip)]
    pub cli_path: Option<String>,
    pub enabled: bool,
    pub timeout_secs: u32,
    /// How long a typed master password is kept in memory (0: only for the
    /// operation that asked).
    pub remember_secs: u32,
}

impl Default for BitwardenConfig {
    fn default() -> Self {
        Self { cli_path: None, enabled: false, timeout_secs: DEFAULT_TIMEOUT_SECS, remember_secs: DEFAULT_REMEMBER_SECS }
    }
}

impl BitwardenConfig {
    pub fn timeout_secs(&self) -> u32 {
        clamp_timeout(self.timeout_secs)
    }

    pub fn remember_secs(&self) -> u32 {
        clamp_remember(self.remember_secs)
    }
}

/// Settings → Password managers → KeePass. No secret is stored: the master
/// password is asked for when a call needs it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct KeepassConfig {
    /// Absolute path to the `.kdbx` file. This device's own setting, never
    /// in the vault: the same path means another file on another computer.
    #[serde(skip)]
    pub database_path: Option<String>,
    /// Absolute path to the database's key file, if it has one. This
    /// device's own setting too.
    #[serde(skip)]
    pub key_file_path: Option<String>,
    pub enabled: bool,
    /// How long a typed master password is kept in memory (0: only for the
    /// operation that asked).
    pub remember_secs: u32,
}

impl Default for KeepassConfig {
    fn default() -> Self {
        Self { database_path: None, key_file_path: None, enabled: false, remember_secs: DEFAULT_REMEMBER_SECS }
    }
}

impl KeepassConfig {
    pub fn remember_secs(&self) -> u32 {
        clamp_remember(self.remember_secs)
    }
}

#[derive(Clone, Default)]
pub struct OnePasswordSecrets {
    pub service_account_token: Option<Zeroizing<String>>,
}

impl fmt::Debug for OnePasswordSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnePasswordSecrets")
            .field("has_service_account_token", &self.service_account_token.is_some())
            .finish()
    }
}

/// Everything Settings → Password managers stores, minus the secrets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct PasswordManagersConfig {
    pub enabled: bool,
    pub passbolt: PassboltConfig,
    pub onepassword: OnePasswordConfig,
    pub bitwarden: BitwardenConfig,
    pub keepass: KeepassConfig,
}

/// The Passbolt secrets, read from the vault for one call and wiped when
/// dropped. Its `Debug` says only which are present.
#[derive(Clone, Default)]
pub struct PassboltSecrets {
    pub private_key: Option<Zeroizing<String>>,
    pub passphrase: Option<Zeroizing<String>>,
    pub totp_secret: Option<Zeroizing<String>>,
}

impl fmt::Debug for PassboltSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PassboltSecrets")
            .field("has_private_key", &self.private_key.is_some())
            .field("has_passphrase", &self.passphrase.is_some())
            .field("has_totp_secret", &self.totp_secret.is_some())
            .finish()
    }
}

/// The configuration and secrets, as read for one call.
#[derive(Clone, Default, Debug)]
pub struct Loaded {
    pub config: PasswordManagersConfig,
    pub passbolt_secrets: PassboltSecrets,
    pub onepassword_secrets: OnePasswordSecrets,
}

/// What `pwmanager_get_config` and `pwmanager_save_config` return. No
/// secrets, only whether each is stored.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PasswordManagersView {
    pub enabled: bool,
    pub passbolt: PassboltView,
    pub onepassword: OnePasswordView,
    pub bitwarden: BitwardenView,
    pub keepass: KeepassView,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KeepassView {
    pub database_path: Option<String>,
    pub key_file_path: Option<String>,
    pub enabled: bool,
    pub remember_secs: u32,
    /// A typed master password is kept in memory right now. Filled in by
    /// the IPC layer.
    pub password_cached: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BitwardenView {
    pub cli_path: Option<String>,
    pub enabled: bool,
    pub timeout_secs: u32,
    pub remember_secs: u32,
    /// A typed master password is kept in memory right now. Filled in by
    /// the IPC layer.
    pub password_cached: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OnePasswordView {
    pub cli_path: Option<String>,
    pub enabled: bool,
    pub auth: OnePasswordAuth,
    pub account: String,
    pub timeout_secs: u32,
    pub has_service_account_token: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PassboltView {
    pub cli_path: Option<String>,
    pub credentials: CredentialsMode,
    pub server_address: String,
    pub mfa_mode: MfaMode,
    pub timeout_secs: u32,
    pub has_private_key: bool,
    pub has_passphrase: bool,
    pub has_totp_secret: bool,
    pub passphrase_mode: PassphraseMode,
    pub passphrase_remember_secs: u32,
    /// An asked passphrase is in memory now for this account ("Forget now"
    /// has something to forget). Filled in by the commands from the
    /// in-memory cache; `false` from `From<&Loaded>` alone.
    pub passphrase_cached: bool,
}

impl From<&Loaded> for PasswordManagersView {
    fn from(l: &Loaded) -> Self {
        let p = &l.config.passbolt;
        Self {
            enabled: l.config.enabled,
            passbolt: PassboltView {
                cli_path: p.cli_path.clone(),
                credentials: p.credentials,
                server_address: p.server_address.clone(),
                mfa_mode: p.mfa_mode,
                timeout_secs: p.timeout_secs(),
                has_private_key: l.passbolt_secrets.private_key.is_some(),
                has_passphrase: l.passbolt_secrets.passphrase.is_some(),
                has_totp_secret: l.passbolt_secrets.totp_secret.is_some(),
                passphrase_mode: p.passphrase_mode(),
                passphrase_remember_secs: p.remember_secs(),
                passphrase_cached: false,
            },
            onepassword: OnePasswordView {
                cli_path: l.config.onepassword.cli_path.clone(),
                enabled: l.config.onepassword.enabled,
                auth: l.config.onepassword.auth,
                account: l.config.onepassword.account.clone(),
                timeout_secs: l.config.onepassword.timeout_secs(),
                has_service_account_token: l.onepassword_secrets.service_account_token.is_some(),
            },
            bitwarden: BitwardenView {
                cli_path: l.config.bitwarden.cli_path.clone(),
                enabled: l.config.bitwarden.enabled,
                timeout_secs: l.config.bitwarden.timeout_secs(),
                remember_secs: l.config.bitwarden.remember_secs(),
                password_cached: false,
            },
            keepass: KeepassView {
                database_path: l.config.keepass.database_path.clone(),
                key_file_path: l.config.keepass.key_file_path.clone(),
                enabled: l.config.keepass.enabled,
                remember_secs: l.config.keepass.remember_secs(),
                password_cached: false,
            },
        }
    }
}

/// What Settings sends to save. A secret left out (or empty) keeps the
/// stored one; `clear_*` removes it. No `Debug`, and wiped when dropped.
#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct PasswordManagersConfigInput {
    #[zeroize(skip)]
    pub enabled: bool,
    pub passbolt: PassboltConfigInput,
    /// Left out (an older window): 1Password's defaults, off.
    #[serde(default)]
    pub onepassword: OnePasswordConfigInput,
    /// Left out (an older window): Bitwarden's defaults, off.
    #[serde(default)]
    pub bitwarden: BitwardenConfigInput,
    /// Left out (an older window): KeePass's defaults, off.
    #[serde(default)]
    pub keepass: KeepassConfigInput,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop, Default)]
pub struct KeepassConfigInput {
    #[serde(default)]
    pub database_path: Option<String>,
    #[serde(default)]
    pub key_file_path: Option<String>,
    #[serde(default)]
    #[zeroize(skip)]
    pub enabled: bool,
    #[serde(default)]
    pub remember_secs: Option<u32>,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop, Default)]
pub struct BitwardenConfigInput {
    #[serde(default)]
    pub cli_path: Option<String>,
    #[serde(default)]
    #[zeroize(skip)]
    pub enabled: bool,
    #[serde(default)]
    pub timeout_secs: Option<u32>,
    #[serde(default)]
    pub remember_secs: Option<u32>,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop, Default)]
pub struct OnePasswordConfigInput {
    #[serde(default)]
    pub cli_path: Option<String>,
    #[serde(default)]
    #[zeroize(skip)]
    pub enabled: bool,
    #[serde(default)]
    #[zeroize(skip)]
    pub auth: OnePasswordAuth,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub timeout_secs: Option<u32>,
    /// A new token; absent or empty keeps the stored one.
    #[serde(default)]
    pub service_account_token: Option<String>,
    #[serde(default)]
    pub clear_service_account_token: bool,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct PassboltConfigInput {
    #[serde(default)]
    pub cli_path: Option<String>,
    #[serde(default)]
    #[zeroize(skip)]
    pub credentials: CredentialsMode,
    #[serde(default)]
    pub server_address: String,
    #[serde(default)]
    #[zeroize(skip)]
    pub mfa_mode: MfaMode,
    #[serde(default)]
    pub timeout_secs: Option<u32>,
    /// Asked for or stored. Asked for means a stored passphrase is removed
    /// by this save: never both. Left out, it is decided as for a
    /// configuration saved before the setting existed: stored if a
    /// passphrase is stored after this save, asked for otherwise.
    #[serde(default)]
    #[zeroize(skip)]
    pub passphrase_mode: Option<PassphraseMode>,
    #[serde(default)]
    pub passphrase_remember_secs: Option<u32>,
    #[serde(default)]
    pub private_key: Option<String>,
    #[serde(default)]
    pub private_key_file: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub totp_secret: Option<String>,
    #[serde(default)]
    pub clear_private_key: bool,
    #[serde(default)]
    pub clear_passphrase: bool,
    #[serde(default)]
    pub clear_totp_secret: bool,
}

/// A change to one stored secret.
pub enum SecretChange {
    Keep,
    Clear,
    Set(Zeroizing<String>),
    /// The passphrase only, when the save asks for it: cleared if the
    /// stored configuration had it stored (the user switched to asking),
    /// kept otherwise. Settled by [`passphrase_effect`] when saving, against
    /// what is stored then.
    ClearIfWasStored,
}

impl fmt::Debug for SecretChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SecretChange::Keep => "Keep",
            SecretChange::Clear => "Clear",
            SecretChange::Set(_) => "Set(<redacted>)",
            SecretChange::ClearIfWasStored => "ClearIfWasStored",
        })
    }
}

/// What a save does to the stored passphrase, given the passphrase mode of
/// the configuration stored before it: [`SecretChange::ClearIfWasStored`]
/// clears it only when leaving the stored mode.
pub fn passphrase_effect(change: &SecretChange, stored_mode: PassphraseMode) -> &SecretChange {
    match change {
        SecretChange::ClearIfWasStored if stored_mode == PassphraseMode::Stored => &SecretChange::Clear,
        SecretChange::ClearIfWasStored => &SecretChange::Keep,
        other => other,
    }
}

/// A checked save: the configuration to store and what to do with each
/// secret. Built before anything is written, so a bad value changes nothing.
#[derive(Debug)]
pub struct ValidatedSave {
    pub config: PasswordManagersConfig,
    pub private_key: SecretChange,
    pub passphrase: SecretChange,
    pub totp_secret: SecretChange,
    pub service_account_token: SecretChange,
}

pub fn clamp_timeout(secs: u32) -> u32 {
    secs.clamp(MIN_TIMEOUT_SECS, MAX_TIMEOUT_SECS)
}

pub fn clamp_remember(secs: u32) -> u32 {
    secs.min(MAX_REMEMBER_SECS)
}

/// The Passbolt server address: `https://` and a host, nothing that could be
/// read as credentials or a second option. Empty is allowed (not set up yet).
/// Returned as the URL parser normalises it (lower-case host, no default
/// port, IDN as punycode), without a trailing slash, so the stored address
/// is exactly the one that was checked.
pub fn validate_server_address(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(String::new());
    }
    if s.chars().count() > MAX_SERVER_ADDRESS_CHARS {
        return Err("The server address is too long".into());
    }
    if s.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("The server address cannot contain spaces or control characters".into());
    }
    let url = reqwest::Url::parse(s).map_err(|_| "The server address is not a valid URL (e.g. https://passbolt.example.com)".to_string())?;
    if url.scheme() != "https" {
        return Err("The server address must start with https://".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("The server address cannot contain a user name or password".into());
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err("The server address has no host name".into());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("The server address cannot have a query (?…) or a fragment (#…)".into());
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}

/// An armored OpenPGP private key, as Passbolt's recovery kit holds it.
pub fn validate_private_key(key: &str) -> Result<(), String> {
    if key.len() as u64 > MAX_PRIVATE_KEY_BYTES {
        return Err("The private key is too large to be an OpenPGP key".into());
    }
    if key.contains('\0') {
        return Err("The private key contains a NUL character".into());
    }
    let begin = key.find(PGP_PRIVATE_BEGIN);
    let end = key.find(PGP_PRIVATE_END);
    match (begin, end) {
        (Some(b), Some(e)) if b < e => Ok(()),
        _ => Err(format!(
            "That is not an armored OpenPGP private key: it must contain \"{PGP_PRIVATE_BEGIN}\" (Passbolt's recovery kit, passbolt-recovery-kit.txt)"
        )),
    }
}

/// Read a private key file, checked the same way as a pasted key.
///
/// The path comes from the webview, so: a network path (`\\server\…`) is
/// refused before it is touched (opening it would send the user's NTLM hash
/// to that server); the file is opened once and everything else is asked of
/// that handle, so it cannot be swapped between the checks and the read; it
/// must be a regular file (not a pipe or a device); and no more than the cap
/// is read, whatever its size claims.
pub fn read_private_key_file(path: &str) -> Result<Zeroizing<String>, String> {
    let trimmed = path.trim();
    if super::cli::is_network_path(trimmed) {
        return Err("The private key file must be on this computer, not on a network share".into());
    }
    let p = Path::new(trimmed);
    if !p.is_absolute() {
        return Err("The private key file must be given with its full path".into());
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        // A FIFO would block the open until someone writes to it.
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(p).map_err(|e| format!("Could not read the private key file: {}", e.kind()))?;
    let meta = file.metadata().map_err(|e| format!("Could not read the private key file: {}", e.kind()))?;
    if !meta.is_file() {
        return Err("The private key path is not a file".into());
    }
    if meta.len() > MAX_PRIVATE_KEY_BYTES {
        return Err("The private key file is too large to be an OpenPGP key".into());
    }
    // Room for one byte past the cap, so a file that grew is noticed without
    // the buffer ever being reallocated (which would leave an unwiped copy).
    let mut bytes = Zeroizing::new(Vec::with_capacity(MAX_PRIVATE_KEY_BYTES as usize + 1));
    file.take(MAX_PRIVATE_KEY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Could not read the private key file: {}", e.kind()))?;
    if bytes.len() as u64 > MAX_PRIVATE_KEY_BYTES {
        return Err("The private key file is too large to be an OpenPGP key".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "The private key file is not text (an armored key is expected)".to_string())?;
    validate_private_key(text)?;
    Ok(Zeroizing::new(text.to_string()))
}

/// A TOTP seed: base32, as Passbolt shows it when MFA is set up. Spaces are
/// dropped and letters upper-cased.
pub fn normalize_totp_secret(raw: &str) -> Result<Zeroizing<String>, String> {
    let s: Zeroizing<String> = Zeroizing::new(
        raw.chars().filter(|c| !c.is_whitespace() && *c != '-').map(|c| c.to_ascii_uppercase()).collect(),
    );
    let body = s.trim_end_matches('=');
    if body.len() < 16 || s.len() > 256 {
        return Err("The TOTP secret should be the base32 seed shown when MFA was set up (at least 16 characters)".into());
    }
    if !body.chars().all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c)) {
        return Err("The TOTP secret must be base32 (letters A–Z and digits 2–7)".into());
    }
    Ok(s)
}

fn validate_passphrase(p: &str) -> Result<(), String> {
    if p.contains('\0') {
        return Err("The passphrase contains a NUL character".into());
    }
    if p.len() > MAX_PASSPHRASE_BYTES {
        return Err("The passphrase is too long".into());
    }
    Ok(())
}

/// The Passbolt CLI path: empty means "find it on PATH"; otherwise an
/// absolute path on this computer to a file named `passbolt` or
/// `passbolt.exe`. See [`validate_cli_path_for`].
pub fn validate_cli_path(raw: Option<&str>, check_file: bool) -> Result<Option<String>, String> {
    validate_cli_path_for(&super::passbolt::SPEC, raw, check_file)
}

/// A manager's CLI path: empty means "find it on PATH"; otherwise an
/// absolute path on this computer to a file named as the tool is.
///
/// `check_file` also looks at the file itself (it exists; on Unix, who owns
/// it). Saving skips that when the feature is off or the path did not change,
/// so a CLI that was uninstalled does not stop the user from switching the
/// feature off; every run checks the file again anyway.
pub(crate) fn validate_cli_path_for(
    spec: &super::cli::CliSpec,
    raw: Option<&str>,
    check_file: bool,
) -> Result<Option<String>, String> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if s.chars().count() > MAX_CLI_PATH_CHARS || s.chars().any(char::is_control) {
        return Err("The CLI path is not valid".into());
    }
    let p = Path::new(s);
    if !super::cli::is_network_path(s) && !p.is_absolute() {
        return Err(format!("The CLI path must be absolute (or leave it empty to find {} on PATH)", spec.program));
    }
    let prefix = format!("{}: ", spec.name);
    let strip = |e: String| e.strip_prefix(prefix.as_str()).map(str::to_string).unwrap_or(e);
    super::cli::check_cli_path_shape(spec, p).map_err(strip)?;
    if check_file {
        super::cli::check_cli_file(spec, p).map_err(strip)?;
    }
    Ok(Some(s.to_string()))
}

/// Check everything Settings sent, reading the key file if one was named.
/// Nothing is stored here. `current_cli_path` is the path this device has
/// stored now: an unchanged path is not looked at again.
pub fn validate_input(
    input: &mut PasswordManagersConfigInput,
    current_cli_path: Option<&str>,
) -> Result<ValidatedSave, String> {
    let device = DeviceSettings { passbolt_cli_path: current_cli_path.map(str::to_string), ..Default::default() };
    validate_input_with(input, &device)
}

/// [`validate_input`], with every CLI path this device has stored now.
pub fn validate_input_with(input: &mut PasswordManagersConfigInput, device: &DeviceSettings) -> Result<ValidatedSave, String> {
    let current_cli_path = device.passbolt_cli_path.as_deref();
    let (onepassword, service_account_token) =
        validate_onepassword(&mut input.onepassword, input.enabled, device.onepassword_cli_path.as_deref())?;
    let keepass = validate_keepass(&input.keepass, input.enabled, device)?;
    let bitwarden = validate_bitwarden(&input.bitwarden, input.enabled, device.bitwarden_cli_path.as_deref())?;
    let p = &mut input.passbolt;
    let new_cli_path = p.cli_path.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let check_cli_file = input.enabled && new_cli_path != current_cli_path.map(str::trim).filter(|s| !s.is_empty());
    let config = PasswordManagersConfig {
        enabled: input.enabled,
        passbolt: PassboltConfig {
            cli_path: validate_cli_path(p.cli_path.as_deref(), check_cli_file)?,
            credentials: p.credentials,
            server_address: validate_server_address(&p.server_address)?,
            mfa_mode: p.mfa_mode,
            timeout_secs: clamp_timeout(p.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS)),
            passphrase_mode: p.passphrase_mode,
            passphrase_remember_secs: clamp_remember(p.passphrase_remember_secs.unwrap_or(DEFAULT_REMEMBER_SECS)),
        },
        onepassword,
        bitwarden,
        keepass,
    };

    let private_key = if p.clear_private_key {
        SecretChange::Clear
    } else if let Some(path) = p.private_key_file.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        SecretChange::Set(read_private_key_file(path)?)
    } else if let Some(key) = p.private_key.take().map(Zeroizing::new).filter(|k| !k.trim().is_empty()) {
        let key = Zeroizing::new(key.trim().to_string());
        validate_private_key(&key)?;
        SecretChange::Set(key)
    } else {
        SecretChange::Keep
    };

    let passphrase = if p.passphrase_mode == Some(PassphraseMode::Ask) {
        // Asked for: a passphrase sent along anyway is not kept, and the
        // stored one goes if this save switches from stored to asked (or
        // its removal was asked for).
        if let Some(mut sent) = p.passphrase.take() {
            sent.zeroize();
        }
        if p.clear_passphrase {
            SecretChange::Clear
        } else {
            SecretChange::ClearIfWasStored
        }
    } else if p.clear_passphrase {
        SecretChange::Clear
    } else if let Some(pass) = p.passphrase.take().map(Zeroizing::new).filter(|s| !s.is_empty()) {
        validate_passphrase(&pass)?;
        SecretChange::Set(pass)
    } else {
        SecretChange::Keep
    };

    let totp_secret = if p.clear_totp_secret {
        SecretChange::Clear
    } else if let Some(seed) = p.totp_secret.take().map(Zeroizing::new).filter(|s| !s.trim().is_empty()) {
        SecretChange::Set(normalize_totp_secret(&seed)?)
    } else {
        SecretChange::Keep
    };

    Ok(ValidatedSave { config, private_key, passphrase, totp_secret, service_account_token })
}

/// The 1Password account name: a sign-in address, shorthand or ID, as the
/// CLI's `--account` takes it. Empty is allowed (the CLI's default).
pub fn validate_account(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if s.chars().count() > MAX_ACCOUNT_CHARS {
        return Err("The 1Password account is too long".into());
    }
    if s.starts_with('-') || s.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("The 1Password account should be its sign-in address (my.1password.com), shorthand or ID".into());
    }
    Ok(s.to_string())
}

/// A 1Password service account token, as 1Password shows it once when the
/// service account is made: `ops_` and a long base64url text.
pub fn validate_service_account_token(raw: &str) -> Result<Zeroizing<String>, String> {
    let s = Zeroizing::new(raw.trim().to_string());
    if !s.starts_with("ops_") || s.len() < 20 {
        return Err("That is not a 1Password service account token (it starts with ops_)".into());
    }
    if s.len() > MAX_SERVICE_ACCOUNT_TOKEN_BYTES || s.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("The service account token is not valid".into());
    }
    Ok(s)
}

fn validate_onepassword(
    o: &mut OnePasswordConfigInput,
    master_enabled: bool,
    current_cli_path: Option<&str>,
) -> Result<(OnePasswordConfig, SecretChange), String> {
    let new_cli_path = o.cli_path.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let check_cli_file =
        master_enabled && o.enabled && new_cli_path != current_cli_path.map(str::trim).filter(|s| !s.is_empty());
    let config = OnePasswordConfig {
        cli_path: validate_cli_path_for(&super::onepassword::SPEC, o.cli_path.as_deref(), check_cli_file)?,
        enabled: o.enabled,
        auth: o.auth,
        account: validate_account(&o.account)?,
        timeout_secs: clamp_timeout(o.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS)),
    };
    let token = if o.clear_service_account_token {
        SecretChange::Clear
    } else if let Some(t) = o.service_account_token.take().map(Zeroizing::new).filter(|t| !t.trim().is_empty()) {
        SecretChange::Set(validate_service_account_token(&t)?)
    } else {
        SecretChange::Keep
    };
    Ok((config, token))
}

fn validate_bitwarden(
    b: &BitwardenConfigInput,
    master_enabled: bool,
    current_cli_path: Option<&str>,
) -> Result<BitwardenConfig, String> {
    let new_cli_path = b.cli_path.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let check_cli_file =
        master_enabled && b.enabled && new_cli_path != current_cli_path.map(str::trim).filter(|s| !s.is_empty());
    Ok(BitwardenConfig {
        cli_path: validate_cli_path_for(&super::bitwarden::SPEC, b.cli_path.as_deref(), check_cli_file)?,
        enabled: b.enabled,
        timeout_secs: clamp_timeout(b.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS)),
        remember_secs: clamp_remember(b.remember_secs.unwrap_or(DEFAULT_REMEMBER_SECS)),
    })
}

/// A KeePass database or key file path: absolute, on this computer, and,
/// when `check_file`, an existing file.
fn validate_keepass_path(raw: Option<&str>, what: &str, check_file: bool) -> Result<Option<String>, String> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if s.chars().count() > MAX_CLI_PATH_CHARS || s.chars().any(char::is_control) {
        return Err(format!("The path of {what} is not valid"));
    }
    let strip = |e: String| e.strip_prefix("KeePass: ").map(str::to_string).unwrap_or(e);
    let path = super::keepass::check_path(s, what).map_err(strip)?;
    if check_file && !path.is_file() {
        return Err(format!("There is no file at {}", path.display()));
    }
    Ok(Some(s.to_string()))
}

fn validate_keepass(k: &KeepassConfigInput, master_enabled: bool, device: &DeviceSettings) -> Result<KeepassConfig, String> {
    let trimmed = |p: Option<&str>| p.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let on = master_enabled && k.enabled;
    // A file is looked at only when its path is new: one that has since
    // gone (a removable drive) does not block saving the rest.
    let database_changed = trimmed(k.database_path.as_deref()) != trimmed(device.keepass_database_path.as_deref());
    let key_file_changed = trimmed(k.key_file_path.as_deref()) != trimmed(device.keepass_key_file_path.as_deref());
    Ok(KeepassConfig {
        database_path: validate_keepass_path(k.database_path.as_deref(), "the database", on && database_changed)?,
        key_file_path: validate_keepass_path(k.key_file_path.as_deref(), "the key file", on && key_file_changed)?,
        enabled: k.enabled,
        remember_secs: clamp_remember(k.remember_secs.unwrap_or(DEFAULT_REMEMBER_SECS)),
    })
}

// ---------------------------------------------------------------------------
// This device's own settings: kept out of the synced vault.

/// The settings file that stays on this device: the CLI path. Next to
/// Reach's other local data; never synced and never put in a vault.
pub fn device_settings_path() -> PathBuf {
    crate::app_data_dir().join("pwmanager").join("device.json")
}

/// What [`device_settings_path`] holds.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DeviceSettings {
    pub passbolt_cli_path: Option<String>,
    pub onepassword_cli_path: Option<String>,
    pub bitwarden_cli_path: Option<String>,
    pub keepass_database_path: Option<String>,
    pub keepass_key_file_path: Option<String>,
}

/// The device settings in `path`. Missing or unreadable: none (find the CLI
/// on PATH).
pub fn read_device_settings(path: &Path) -> DeviceSettings {
    match std::fs::read(path) {
        Ok(raw) => serde_json::from_slice(&raw).unwrap_or_else(|e| {
            tracing::warn!("pwmanager: {} is unreadable, ignoring it: {e}", path.display());
            DeviceSettings::default()
        }),
        Err(_) => DeviceSettings::default(),
    }
}

/// Store the device settings in `path`: written beside it and renamed over
/// it, so a crash leaves the old file or the new one.
pub fn write_device_settings(path: &Path, settings: &DeviceSettings) -> Result<(), String> {
    let dir = path.parent().ok_or("No folder for the device settings")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let json = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not write {}: {e}", path.display())
    })
}

// ---------------------------------------------------------------------------
// The vault.

/// The configuration and secrets. A locked vault is an error: nothing can be
/// fetched without the key and passphrase it holds.
pub async fn load(state: &AppState) -> Result<Loaded, String> {
    let manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("The vault is locked. Unlock Reach first.".into());
    }
    let mut config = read_setting_by_key(&manager, CONFIG_KEY)
        .await
        .and_then(|raw| parse_config(&raw))
        .unwrap_or_default();
    let device = read_device_settings(&device_settings_path());
    let device_path = |p: Option<String>| p.filter(|s| !s.trim().is_empty());
    config.passbolt.cli_path = device_path(device.passbolt_cli_path);
    config.onepassword.cli_path = device_path(device.onepassword_cli_path);
    config.bitwarden.cli_path = device_path(device.bitwarden_cli_path);
    config.keepass.database_path = device_path(device.keepass_database_path);
    config.keepass.key_file_path = device_path(device.keepass_key_file_path);
    let secret = |v: Option<String>| v.map(Zeroizing::new).filter(|s| !s.is_empty());
    let passbolt_secrets = PassboltSecrets {
        private_key: secret(read_setting_by_key(&manager, PASSBOLT_PRIVATE_KEY).await),
        passphrase: secret(read_setting_by_key(&manager, PASSBOLT_PASSPHRASE).await),
        totp_secret: secret(read_setting_by_key(&manager, PASSBOLT_TOTP_SECRET).await),
    };
    config.passbolt.settle_passphrase_mode(passbolt_secrets.passphrase.is_some());
    let onepassword_secrets = OnePasswordSecrets {
        service_account_token: secret(read_setting_by_key(&manager, ONEPASSWORD_SERVICE_ACCOUNT_TOKEN).await),
    };
    Ok(Loaded { config, passbolt_secrets, onepassword_secrets })
}

/// The stored configuration JSON. Unreadable means "as shipped": off.
pub fn parse_config(raw: &str) -> Option<PasswordManagersConfig> {
    match serde_json::from_str::<PasswordManagersConfig>(raw) {
        Ok(mut c) => {
            c.passbolt.timeout_secs = clamp_timeout(c.passbolt.timeout_secs);
            c.passbolt.passphrase_remember_secs = clamp_remember(c.passbolt.passphrase_remember_secs);
            c.onepassword.timeout_secs = clamp_timeout(c.onepassword.timeout_secs);
            c.bitwarden.timeout_secs = clamp_timeout(c.bitwarden.timeout_secs);
            c.bitwarden.remember_secs = clamp_remember(c.bitwarden.remember_secs);
            c.keepass.remember_secs = clamp_remember(c.keepass.remember_secs);
            Some(c)
        }
        Err(e) => {
            tracing::warn!("pwmanager: stored configuration unreadable, using defaults: {e}");
            None
        }
    }
}

async fn apply_secret(
    manager: &mut crate::vault::VaultManager,
    key: &str,
    change: &SecretChange,
) -> Result<(), String> {
    match change {
        // Settled by `passphrase_effect` before it gets here; unsettled, it
        // deletes nothing.
        SecretChange::Keep | SecretChange::ClearIfWasStored => Ok(()),
        SecretChange::Clear => delete_setting_by_key(manager, key).await,
        SecretChange::Set(value) => write_setting_by_key(manager, key, value).await,
    }
}

/// The passphrase mode of the configuration stored now, decided as [`load`]
/// does for one saved before the setting existed.
async fn stored_passphrase_mode(manager: &crate::vault::VaultManager) -> PassphraseMode {
    let mut stored = read_setting_by_key(manager, CONFIG_KEY)
        .await
        .and_then(|raw| parse_config(&raw))
        .unwrap_or_default();
    let has_stored = read_setting_by_key(manager, PASSBOLT_PASSPHRASE).await.map(Zeroizing::new).is_some_and(|s| !s.is_empty());
    stored.passbolt.settle_passphrase_mode(has_stored);
    stored.passbolt.passphrase_mode()
}

/// Store a checked save.
pub async fn save(state: &AppState, save: &ValidatedSave) -> Result<(), String> {
    let json = serde_json::to_string(&save.config).map_err(|e| e.to_string())?;
    let mut manager = state.vault_manager.lock().await;
    if manager.is_locked() {
        return Err("The vault is locked. Unlock Reach first.".into());
    }
    // Before the new configuration replaces it.
    let stored_mode = stored_passphrase_mode(&manager).await;
    // The secrets first: if removing one fails, the configuration that says
    // it is gone ("Ask when needed") is not written, so what Settings shows
    // is never less than what is stored.
    apply_secret(&mut manager, PASSBOLT_PRIVATE_KEY, &save.private_key).await?;
    apply_secret(&mut manager, PASSBOLT_PASSPHRASE, passphrase_effect(&save.passphrase, stored_mode)).await?;
    apply_secret(&mut manager, PASSBOLT_TOTP_SECRET, &save.totp_secret).await?;
    apply_secret(&mut manager, ONEPASSWORD_SERVICE_ACCOUNT_TOKEN, &save.service_account_token).await?;
    write_setting_by_key(&mut manager, CONFIG_KEY, &json).await?;
    drop(manager);
    let path = device_settings_path();
    let mut device = read_device_settings(&path);
    let wanted = DeviceSettings {
        passbolt_cli_path: save.config.passbolt.cli_path.clone(),
        onepassword_cli_path: save.config.onepassword.cli_path.clone(),
        bitwarden_cli_path: save.config.bitwarden.cli_path.clone(),
        keepass_database_path: save.config.keepass.database_path.clone(),
        keepass_key_file_path: save.config.keepass.key_file_path.clone(),
    };
    if device != wanted {
        device = wanted;
        write_device_settings(&path, &device)?;
    }
    tracing::info!(
        "pwmanager: saved configuration enabled={} credentials={:?} mfa={:?} passphrase={:?} remember={}s",
        save.config.enabled,
        save.config.passbolt.credentials,
        save.config.passbolt.mfa_mode,
        save.config.passbolt.passphrase_mode(),
        save.config.passbolt.remember_secs()
    );
    tracing::info!(
        "pwmanager: 1password enabled={} auth={:?} account_set={}",
        save.config.onepassword.enabled,
        save.config.onepassword.auth,
        !save.config.onepassword.account.is_empty()
    );
    tracing::info!(
        "pwmanager: bitwarden enabled={} remember={}s",
        save.config.bitwarden.enabled,
        save.config.bitwarden.remember_secs()
    );
    tracing::info!(
        "pwmanager: keepass enabled={} database_set={} key_file_set={} remember={}s",
        save.config.keepass.enabled,
        save.config.keepass.database_path.is_some(),
        save.config.keepass.key_file_path.is_some(),
        save.config.keepass.remember_secs()
    );
    Ok(())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
