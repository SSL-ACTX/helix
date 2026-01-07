use anyhow::Result;
use argon2::{
    Argon2, Params, Algorithm, Version
};
use hkdf::Hkdf;
use sha2::Sha256;
use aes_gcm::{Key, Aes256Gcm};

/// SLOW: Derives a Master Key from the user password (runs once at startup).
///
/// Uses Argon2id (Memory-Hard) to prevent GPU/ASIC brute-force attacks.
/// Config: 16MB RAM, 3 Iterations, 1 Parallel Lane.
pub fn derive_master_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let params = Params::new(16 * 1024, 3, 1, Some(32)).unwrap();
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key_out = [0u8; 32];
    argon2.hash_password_into(password.as_bytes(), salt, &mut key_out)
    .map_err(|e| anyhow::anyhow!("Master Key derivation failed: {}", e))?;

    Ok(key_out)
}

/// FAST: Derives a unique Session Key for a specific 32MB block.
///
/// Uses HKDF-SHA256 to combine the Master Key with a unique Block Salt.
/// This ensures that identical files result in different DNA sequences.
pub fn derive_session_key(master_key: &[u8], block_salt: &[u8]) -> Key<Aes256Gcm> {
    let hk = Hkdf::<Sha256>::new(Some(block_salt), master_key);
    let mut okm = [0u8; 32];
    hk.expand(&[], &mut okm).expect("HKDF expansion failed");
    *Key::<Aes256Gcm>::from_slice(&okm)
}
