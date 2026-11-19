//! Secrets behind Face ID or Touch ID on iOS: a keychain item whose access
//! control asks for the current set of enrolled faces or fingers on every
//! read (`kSecAccessControlBiometryCurrentSet`), kept on this device only
//! and only while a passcode is set. Writing needs no check; reading shows
//! the system prompt and blocks until it is answered. Enrolling another face
//! or finger makes the item unreadable for good, by design.
//!
//! iOS gives every app its own keychain access group, so unlike the macOS
//! build (see [`super::biometric`]) the item really is bound to the
//! biometric in hardware, not only gated by a check before it is read.

/// Why a read failed.
#[derive(Debug, PartialEq, Eq)]
pub enum ReadError {
    /// Nothing there, or no longer readable (the enrolled set changed).
    Gone,
    /// The user cancelled or did not pass the check.
    Declined(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Gone => f.write_str("The biometric item is gone"),
            ReadError::Declined(m) => f.write_str(m),
        }
    }
}

#[cfg(target_os = "ios")]
mod imp {
    use super::ReadError;
    use objc2_local_authentication::{LABiometryType, LAContext, LAPolicy};
    use security_framework::access_control::{ProtectionMode, SecAccessControl};
    use security_framework::passwords::{
        delete_generic_password_options, generic_password, set_generic_password_options, AccessControlOptions,
        PasswordOptions,
    };
    use zeroize::Zeroizing;

    const ERR_ITEM_NOT_FOUND: i32 = -25300;

    /// Face ID or Touch ID is set up, and a passcode with it.
    pub fn available() -> bool {
        // SAFETY: a fresh context asked whether the policy can be evaluated.
        unsafe { LAContext::new().canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthenticationWithBiometrics).is_ok() }
    }

    /// "Face ID", "Touch ID" or "Optic ID", as this device has.
    pub fn name() -> &'static str {
        // SAFETY: biometryType is read after canEvaluatePolicy, as Apple asks.
        unsafe {
            let context = LAContext::new();
            let _ = context.canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthenticationWithBiometrics);
            match context.biometryType() {
                LABiometryType::TouchID => "Touch ID",
                LABiometryType::OpticID => "Optic ID",
                _ => "Face ID",
            }
        }
    }

    pub fn store(service: &str, account: &str, secret: &[u8]) -> Result<(), String> {
        // Replacing an item in place would have to read it first, which
        // asks; deleting needs no check.
        let _ = delete_generic_password_options(PasswordOptions::new_generic_password(service, account));
        let access = SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenPasscodeSetThisDeviceOnly),
            AccessControlOptions::BIOMETRY_CURRENT_SET.bits(),
        )
        .map_err(|e| format!("Face ID protection is not available: {e}"))?;
        let mut options = PasswordOptions::new_generic_password(service, account);
        options.set_access_control(access);
        set_generic_password_options(secret, options).map_err(|e| e.to_string())
    }

    /// Blocking: shows the system prompt.
    pub fn read(service: &str, account: &str) -> Result<Zeroizing<Vec<u8>>, ReadError> {
        match generic_password(PasswordOptions::new_generic_password(service, account)) {
            Ok(secret) => Ok(Zeroizing::new(secret)),
            Err(e) if e.code() == ERR_ITEM_NOT_FOUND => Err(ReadError::Gone),
            Err(e) => Err(ReadError::Declined(e.to_string())),
        }
    }

    pub fn delete(service: &str, account: &str) {
        let _ = delete_generic_password_options(PasswordOptions::new_generic_password(service, account));
    }
}

#[cfg(not(target_os = "ios"))]
mod imp {
    use super::ReadError;
    use zeroize::Zeroizing;

    pub fn available() -> bool {
        false
    }

    pub fn name() -> &'static str {
        "Face ID"
    }

    pub fn store(_: &str, _: &str, _: &[u8]) -> Result<(), String> {
        Err("Face ID is only on iOS".into())
    }

    pub fn read(_: &str, _: &str) -> Result<Zeroizing<Vec<u8>>, ReadError> {
        Err(ReadError::Gone)
    }

    pub fn delete(_: &str, _: &str) {}
}

pub use imp::*;
