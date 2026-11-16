use argon2::{Algorithm, Argon2, Params, Version};

use crate::vault::error::VaultError;
use crate::vault::types::Kek;

/// Argon2id parameters for key derivation.
/// - 256 MiB memory (as specified)
/// - 4 iterations
/// - 4 parallelism
/// - 32-byte output
const MEMORY_COST: u32 = 262_144; // 256 MiB
const TIME_COST: u32 = 4;
const PARALLELISM: u32 = 4;
const OUTPUT_LEN: usize = 32;

/// Derive a 32-byte KEK from a password using Argon2id.
pub fn derive_kek(password: &[u8], salt: &[u8; 32]) -> Result<Kek, VaultError> {
    let params = Params::new(MEMORY_COST, TIME_COST, PARALLELISM, Some(OUTPUT_LEN))
        .map_err(|e| VaultError::KdfError(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    Kek::try_from_fn(|key| {
        argon2
            .hash_password_into(password, salt, key)
            .map_err(|e| VaultError::KdfError(e.to_string()))
    })
}

/// Generate a random 32-byte salt.
pub fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    rand::fill(&mut salt[..]);
    salt
}

#[cfg(test)]
#[path = "kdf_tests.rs"]
mod tests;
