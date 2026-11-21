use super::{desktop_seal, encode, load, write_file, Entry, OpenError, FILE};
use crate::ipc::vault_commands::PromptText;
use crate::ssh::unlocked;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::path::Path;
use tauri::AppHandle;
use zeroize::Zeroizing;

type Key = Zeroizing<[u8; 32]>;

pub async fn open(
    _app: &AppHandle,
    dir: &Path,
    _prompt: &PromptText,
) -> Result<Vec<Entry>, OpenError> {
    let saved = load(dir).filter(|s| s.count > 0).ok_or(OpenError::Gone)?;
    tokio::task::spawn_blocking(move || {
        let challenge = BASE64
            .decode(&saved.challenge)
            .map_err(|e| OpenError::Declined(e.to_string()))?;
        let key = device::key(&challenge, false)
            .map_err(OpenError::Declined)?
            .ok_or(OpenError::Gone)?;
        desktop_seal::open(saved, &key).map_err(OpenError::Declined)
    })
    .await
    .map_err(|e| OpenError::Declined(e.to_string()))?
}

pub async fn save(
    _app: &AppHandle,
    dir: &Path,
    entries: &[Entry],
    _prompt: &PromptText,
) -> Result<(), String> {
    let bytes = encode(entries)?;
    let count = entries.len();
    let dir = dir.to_path_buf();
    let revision = unlocked::revision();
    tokio::task::spawn_blocking(move || {
        let challenge: [u8; 32] = rand::random();
        let key = device::key(&challenge, true)?.ok_or("The device unlock key is missing")?;
        let saved = desktop_seal::seal(&bytes, count, &challenge, &key)?;
        if unlocked::revision() != revision {
            return Err("The startup key unlock round ended during device authentication".into());
        }
        write_file(&dir, &saved)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn forget(_app: &AppHandle, dir: &Path) -> Result<(), String> {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let remove = |path| match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        };
        // Attempt each removal even if one fails.
        remove(dir.join(FILE))
            .and(remove(dir.join(format!("{FILE}.tmp"))))
            .and(device::forget())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(windows)]
mod device {
    use super::Key;
    use crate::vault::biometric::hello;

    const NAME: &str = "Reacher CM SSH key passphrases";

    pub(super) fn key(challenge: &[u8], create: bool) -> Result<Option<Key>, String> {
        if challenge.len() != 32 {
            return Err("The Windows Hello challenge is damaged".into());
        }
        hello::key_for_name(NAME, challenge, create)
    }

    pub(super) fn forget() -> Result<(), String> {
        hello::forget_name(NAME)
    }
}

#[cfg(target_os = "macos")]
pub(super) use device::storage_available;

#[cfg(target_os = "macos")]
mod device {
    use super::{Key, Zeroizing};
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::string::{CFString, CFStringRef};
    use security_framework::access_control::{ProtectionMode, SecAccessControl};
    use security_framework::passwords::{
        delete_generic_password_options, generic_password, set_generic_password_options,
        AccessControlOptions, PasswordOptions,
    };

    const SERVICE: &str = "reachercm-ssh-key-touch-id";
    const ACCOUNT: &str = "wrapping-key";
    const ITEM_NOT_FOUND: i32 = -25300;

    fn options(account: &str) -> Result<PasswordOptions, String> {
        // This constant first appeared in macOS 10.15. Resolve it at runtime
        // so older systems can still launch the app and type passphrases.
        // SAFETY: Security exports this symbol as a static CFStringRef.
        let protection = unsafe {
            let symbol = libc::dlsym(
                libc::RTLD_DEFAULT,
                c"kSecUseDataProtectionKeychain".as_ptr(),
            );
            if symbol.is_null() {
                return Err("The Data Protection Keychain requires macOS 10.15 or later".into());
            }
            CFString::wrap_under_get_rule(*symbol.cast::<CFStringRef>())
        };
        let mut options = PasswordOptions::new_generic_password(SERVICE, account);
        // The typed setter requires linking the constant at build time.
        #[allow(deprecated)]
        options
            .query
            .push((protection, CFBoolean::from(true).into_CFType()));
        options.set_access_synchronized(Some(false));
        Ok(options)
    }

    pub(crate) fn storage_available() -> bool {
        // A query for a reserved account that is never written checks access
        // to the store without triggering Touch ID or creating a credential.
        options("availability-probe").is_ok_and(
            |probe| matches!(generic_password(probe), Err(e) if e.code() == ITEM_NOT_FOUND),
        )
    }

    fn read() -> Result<Option<Key>, String> {
        match generic_password(options(ACCOUNT)?) {
            Ok(bytes) => {
                let bytes = Zeroizing::new(bytes);
                if bytes.len() != 32 {
                    return Err("The Touch ID key is damaged".into());
                }
                let mut key = Zeroizing::new([0; 32]);
                key.copy_from_slice(&bytes);
                Ok(Some(key))
            }
            Err(e) if e.code() == ITEM_NOT_FOUND => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub(super) fn key(_challenge: &[u8], create: bool) -> Result<Option<Key>, String> {
        if let Some(key) = read()? {
            return Ok(Some(key));
        }
        if !create {
            return Ok(None);
        }
        let key: Key = Zeroizing::new(rand::random());
        let access = SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
            AccessControlOptions::BIOMETRY_CURRENT_SET.bits(),
        )
        .map_err(|e| e.to_string())?;
        // An invalidated item can be hidden from reads while it still exists.
        forget()?;
        let mut item = options(ACCOUNT)?;
        item.set_access_control(access);
        set_generic_password_options(&*key, item).map_err(|e| e.to_string())?;
        // Prove the shipped app can read the item with Touch ID before saving.
        match read() {
            Ok(Some(check)) if *check == *key => Ok(Some(key)),
            result => {
                let _ = forget();
                Err(result
                    .err()
                    .unwrap_or_else(|| "The Touch ID key could not be read back".into()))
            }
        }
    }

    pub(super) fn forget() -> Result<(), String> {
        match delete_generic_password_options(options(ACCOUNT)?) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ITEM_NOT_FOUND => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
