use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose, Engine as _};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;

const PBKDF2_ITERATIONS: u32 = 600_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Derive a 32-byte AES key from a user password and salt using PBKDF2-SHA256.
fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

/// Compute SHA-256 hash of the given data, returned as hex string.
pub fn snapshot_hash(data: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Encrypt plaintext bytes with a user password.
/// Format: [16 bytes salt][12 bytes nonce][ciphertext + GCM tag]
pub fn encrypt(password: &str, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; SALT_LEN];
    rand::rng().fill_bytes(&mut salt);

    let key = derive_key(password, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("[SYNC_CRYPTO_ERROR] Cipher init: {e}"))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("[SYNC_CRYPTO_ERROR] Encryption failed: {e}"))?;

    let mut output = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypt data encrypted by `encrypt`. Returns plaintext bytes.
pub fn decrypt(password: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < SALT_LEN + NONCE_LEN + 16 {
        return Err("[SYNC_CRYPTO_ERROR] Data too short".to_string());
    }

    let (salt, rest) = data.split_at(SALT_LEN);
    let (nonce_bytes, ciphertext) = rest.split_at(NONCE_LEN);

    let key = derive_key(password, salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("[SYNC_CRYPTO_ERROR] Cipher init: {e}"))?;

    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| {
            format!(
                "[SYNC_PASSWORD_ERROR] Decryption failed (wrong password?): {e}"
            )
        })
}

/// Encrypt a string value for local storage using the given key material.
/// Used for encrypting provider credentials before saving to sync_state.
pub fn encrypt_with_key(key: &[u8; 32], plaintext: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("[SYNC_CRYPTO_ERROR] Cipher init: {e}"))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("[SYNC_CRYPTO_ERROR] Encryption failed: {e}"))?;

    let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&ciphertext);
    Ok(format!(
        "enc:sync:{}",
        general_purpose::STANDARD.encode(payload)
    ))
}

/// Decrypt a string value that was encrypted with `encrypt_with_key`.
pub fn decrypt_with_key(key: &[u8; 32], encrypted: &str) -> Result<String, String> {
    let prefix = "enc:sync:";
    if !encrypted.starts_with(prefix) {
        return Err("[SYNC_CRYPTO_ERROR] Invalid encrypted format".to_string());
    }
    let b64 = &encrypted[prefix.len()..];
    let payload = general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("[SYNC_CRYPTO_ERROR] Base64 decode: {e}"))?;
    if payload.len() < NONCE_LEN + 16 {
        return Err("[SYNC_CRYPTO_ERROR] Payload too short".to_string());
    }
    let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("[SYNC_CRYPTO_ERROR] Cipher init: {e}"))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("[SYNC_CRYPTO_ERROR] Decryption failed: {e}"))?;
    String::from_utf8(plaintext).map_err(|e| format!("[SYNC_CRYPTO_ERROR] UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let password = "test-sync-password-123";
        let plaintext = br#"{"version":1,"data":{"connections":[]}}"#;
        let encrypted = encrypt(password, plaintext).unwrap();
        let decrypted = decrypt(password, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_with_wrong_password_fails() {
        let encrypted = encrypt("correct-password", b"secret data").unwrap();
        let result = decrypt("wrong-password", &encrypted);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("[SYNC_PASSWORD_ERROR]"));
    }

    #[test]
    fn snapshot_hash_is_deterministic() {
        let data = b"hello world";
        let h1 = snapshot_hash(data);
        let h2 = snapshot_hash(data);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn encrypt_decrypt_with_key_round_trip() {
        let mut key = [0u8; 32];
        rand::rng().fill_bytes(&mut key);
        let encrypted = encrypt_with_key(&key, "secret-value").unwrap();
        let decrypted = decrypt_with_key(&key, &encrypted).unwrap();
        assert_eq!(decrypted, "secret-value");
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let mut key1 = [0u8; 32];
        let mut key2 = [0u8; 32];
        rand::rng().fill_bytes(&mut key1);
        rand::rng().fill_bytes(&mut key2);
        let encrypted = encrypt_with_key(&key1, "secret").unwrap();
        let result = decrypt_with_key(&key2, &encrypted);
        assert!(result.is_err());
    }
}
