use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{Engine as _, engine::general_purpose};
use std::env;

#[derive(Clone)]
pub struct TokenCrypto {
    cipher: Aes256Gcm,
}

impl TokenCrypto {
    pub fn new() -> Result<Self> {
        let key_string = env::var("TOKEN_ENCRYPTION_KEY")
            .map_err(|_| anyhow!("TOKEN_ENCRYPTION_KEY environment variable not set"))?;
        
        let key_bytes = general_purpose::STANDARD
            .decode(key_string)
            .map_err(|_| anyhow!("Invalid TOKEN_ENCRYPTION_KEY format"))?;
        
        if key_bytes.len() != 32 {
            return Err(anyhow!("TOKEN_ENCRYPTION_KEY must be 32 bytes when base64 decoded"));
        }
        
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self.cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;
        
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        
        Ok(general_purpose::STANDARD.encode(combined))
    }

    pub fn decrypt(&self, encrypted: &str) -> Result<String> {
        let combined = general_purpose::STANDARD
            .decode(encrypted)
            .map_err(|_| anyhow!("Invalid encrypted token format"))?;
        
        if combined.len() < 12 {
            return Err(anyhow!("Encrypted token too short"));
        }
        
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        let plaintext = self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;
        
        String::from_utf8(plaintext)
            .map_err(|_| anyhow!("Decrypted data is not valid UTF-8"))
    }

    pub fn generate_key() -> String {
        let key = Aes256Gcm::generate_key(OsRng);
        general_purpose::STANDARD.encode(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_encryption_roundtrip() {
        // Set a test key
        let test_key = TokenCrypto::generate_key();
        env::set_var("TOKEN_ENCRYPTION_KEY", &test_key);

        let crypto = TokenCrypto::new().unwrap();
        let original = "ya29.a0AcM612xKwGxTUWg...test_token";
        
        let encrypted = crypto.encrypt(original).unwrap();
        assert_ne!(encrypted, original);
        
        let decrypted = crypto.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_key_generation() {
        let key = TokenCrypto::generate_key();
        assert_eq!(key.len(), 44); // 32 bytes = 44 characters in base64
        
        // Verify it can be decoded
        let decoded = general_purpose::STANDARD.decode(&key).unwrap();
        assert_eq!(decoded.len(), 32);
    }
}
