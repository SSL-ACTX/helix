use anyhow::Result;
use argon2::{
    Argon2, Params, Algorithm, Version
};
use hkdf::Hkdf;
use sha2::Sha256;
use chacha20poly1305::Key;

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
pub fn derive_session_key(master_key: &[u8], block_salt: &[u8]) -> Key {
    let hk = Hkdf::<Sha256>::new(Some(block_salt), master_key);
    let mut okm = [0u8; 32];
    hk.expand(&[], &mut okm).expect("HKDF expansion failed");
    *Key::from_slice(&okm)
}

#[cfg(test)]
mod tests {
    use super::{derive_master_key, derive_session_key};

    #[test]
    fn master_key_is_deterministic_for_same_inputs() {
        let salt = [7u8; 16];
        let k1 = derive_master_key("password", &salt).expect("derive failed");
        let k2 = derive_master_key("password", &salt).expect("derive failed");
        assert_eq!(k1, k2);
    }

    #[test]
    fn master_key_changes_with_salt() {
        let salt_a = [1u8; 16];
        let salt_b = [2u8; 16];
        let k1 = derive_master_key("password", &salt_a).expect("derive failed");
        let k2 = derive_master_key("password", &salt_b).expect("derive failed");
        assert_ne!(k1, k2);
    }

    #[test]
    fn session_key_is_deterministic() {
        let master = [3u8; 32];
        let salt = [9u8; 16];
        let k1 = derive_session_key(&master, &salt);
        let k2 = derive_session_key(&master, &salt);
        assert_eq!(k1.as_slice(), k2.as_slice());
    }

    #[test]
    fn session_key_changes_with_salt() {
        let master = [3u8; 32];
        let salt_a = [9u8; 16];
        let salt_b = [8u8; 16];
        let k1 = derive_session_key(&master, &salt_a);
        let k2 = derive_session_key(&master, &salt_b);
        assert_ne!(k1.as_slice(), k2.as_slice());
    }
}
