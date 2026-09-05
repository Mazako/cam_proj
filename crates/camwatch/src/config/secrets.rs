use std::env;

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use thiserror::Error;

const PREFIX: &str = "enc:v1:aes256gcm:";
const NONCE_LENGTH: usize = 12;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("CAMWATCH_CONFIG_KEY is missing")]
    MissingKey,
    #[error("CAMWATCH_CONFIG_KEY must be a Base64-encoded 32-byte key")]
    InvalidKey,
    #[error("secret value has an unsupported format")]
    UnsupportedFormat,
    #[error("secret value is not valid Base64")]
    InvalidBase64,
    #[error("secret value is invalid or has been tampered with")]
    InvalidCiphertext,
    #[error("secret value is not valid UTF-8")]
    InvalidUtf8,
    #[error("secret value could not be encrypted")]
    EncryptionFailed,
}

#[derive(Clone)]
pub struct SecretManager {
    cipher: Aes256Gcm,
}

impl SecretManager {
    pub fn from_environment() -> Result<Self, SecretError> {
        let encoded_key = env::var("CAMWATCH_CONFIG_KEY").map_err(|_| SecretError::MissingKey)?;
        let key = STANDARD
            .decode(encoded_key)
            .map_err(|_| SecretError::InvalidKey)?;
        let key: [u8; 32] = key.try_into().map_err(|_| SecretError::InvalidKey)?;
        Ok(Self::from_key(key))
    }

    pub fn from_key(key: [u8; 32]) -> Self {
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key);
        Self {
            cipher: Aes256Gcm::new(key),
        }
    }

    pub fn encrypt(&self, value: &str) -> Result<String, SecretError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, value.as_bytes())
            .map_err(|_| SecretError::EncryptionFailed)?;
        let mut payload = nonce.to_vec();
        payload.extend_from_slice(&ciphertext);
        Ok(format!("{PREFIX}{}", STANDARD.encode(payload)))
    }

    pub fn decrypt(&self, value: &str) -> Result<String, SecretError> {
        let encoded = value
            .strip_prefix(PREFIX)
            .ok_or(SecretError::UnsupportedFormat)?;
        let payload = STANDARD
            .decode(encoded)
            .map_err(|_| SecretError::InvalidBase64)?;
        if payload.len() <= NONCE_LENGTH {
            return Err(SecretError::InvalidCiphertext);
        }
        let (nonce, ciphertext) = payload.split_at(NONCE_LENGTH);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| SecretError::InvalidCiphertext)?;
        String::from_utf8(plaintext).map_err(|_| SecretError::InvalidUtf8)
    }
}

#[cfg(test)]
mod tests {
    use super::{PREFIX, SecretError, SecretManager};

    #[test]
    fn encrypts_and_decrypts_a_value() {
        let manager = SecretManager::from_key([7; 32]);
        let encrypted = manager.encrypt("rtsp://camera/stream").unwrap();

        assert!(encrypted.starts_with(PREFIX));
        assert_eq!(manager.decrypt(&encrypted).unwrap(), "rtsp://camera/stream");
    }

    #[test]
    fn encrypting_the_same_value_twice_produces_different_ciphertexts() {
        let manager = SecretManager::from_key([7; 32]);

        assert_ne!(
            manager.encrypt("same value").unwrap(),
            manager.encrypt("same value").unwrap()
        );
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let manager = SecretManager::from_key([7; 32]);
        let encrypted = manager.encrypt("secret").unwrap();
        let mut bytes = encrypted.into_bytes();
        let last = bytes.len() - 1;
        bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(bytes).unwrap();

        assert!(matches!(
            manager.decrypt(&tampered),
            Err(SecretError::InvalidCiphertext | SecretError::InvalidBase64)
        ));
    }
}
