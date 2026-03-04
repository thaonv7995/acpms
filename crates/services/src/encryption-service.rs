use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;

/// EncryptionService provides AES-256-GCM encryption for sensitive data.
///
/// ## Security Properties
/// - Algorithm: AES-256-GCM (authenticated encryption)
/// - Key Size: 256 bits (32 bytes)
/// - Nonce Size: 96 bits (12 bytes) - randomly generated per encryption
/// - Authentication: Built-in via GCM mode
///
/// ## Usage
/// ```ignore
/// let service = EncryptionService::from_env()?;
/// let encrypted = service.encrypt("sensitive-data")?;
/// let decrypted = service.decrypt(&encrypted)?;
/// ```
#[derive(Clone)]
pub struct EncryptionService {
    cipher: Aes256Gcm,
}

impl EncryptionService {
    /// Create a new EncryptionService from a base64-encoded key.
    ///
    /// ## Key Requirements
    /// - Must be exactly 32 bytes (256 bits) when decoded
    /// - Should be cryptographically random
    /// - Must be stored securely (ENV vars, AWS Secrets Manager, Vault)
    ///
    /// ## Example
    /// Generate a key: `openssl rand -base64 32`
    pub fn new(key_base64: &str) -> Result<Self> {
        let key_bytes = BASE64
            .decode(key_base64)
            .context("Failed to decode base64 encryption key")?;

        if key_bytes.len() != 32 {
            bail!(
                "Invalid encryption key length: expected 32 bytes, got {}",
                key_bytes.len()
            );
        }

        let cipher =
            Aes256Gcm::new_from_slice(&key_bytes).context("Failed to create AES-256-GCM cipher")?;

        Ok(Self { cipher })
    }

    /// Create EncryptionService from ENCRYPTION_KEY environment variable.
    ///
    /// ## Security Note
    /// In production, retrieve from secure secret storage (AWS Secrets Manager, Vault)
    /// rather than plain environment variables.
    pub fn from_env() -> Result<Self> {
        let key = std::env::var("ENCRYPTION_KEY")
            .context("ENCRYPTION_KEY environment variable not set")?;
        Self::new(&key)
    }

    /// Encrypt plaintext using AES-256-GCM.
    ///
    /// ## Process
    /// 1. Generate random 12-byte nonce
    /// 2. Encrypt plaintext with nonce
    /// 3. Prepend nonce to ciphertext
    /// 4. Base64 encode the result
    ///
    /// ## Output Format
    /// Base64(nonce || ciphertext || auth_tag)
    ///
    /// ## Security
    /// - Nonce is never reused (randomly generated per call)
    /// - Authentication tag prevents tampering
    /// - No information leakage from error messages
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        // Generate random nonce (12 bytes for GCM)
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        // Prepend nonce to ciphertext (needed for decryption)
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);

        // Base64 encode
        Ok(BASE64.encode(result))
    }

    /// Decrypt ciphertext encrypted with `encrypt()`.
    ///
    /// ## Process
    /// 1. Base64 decode input
    /// 2. Extract nonce (first 12 bytes)
    /// 3. Extract ciphertext (remaining bytes)
    /// 4. Decrypt and verify authentication tag
    ///
    /// ## Error Cases
    /// - Invalid base64
    /// - Corrupted ciphertext
    /// - Wrong encryption key
    /// - Tampered data (auth tag verification fails)
    ///
    /// ## Security
    /// All error cases return generic error to prevent oracle attacks
    pub fn decrypt(&self, ciphertext_base64: &str) -> Result<String> {
        // Base64 decode
        let data = BASE64
            .decode(ciphertext_base64)
            .context("Failed to decode base64 ciphertext")?;

        // Extract nonce (first 12 bytes)
        if data.len() < 12 {
            bail!("Invalid ciphertext: too short");
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext_bytes = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("Decryption failed"))?;

        // Convert to string
        String::from_utf8(plaintext_bytes).context("Decrypted data is not valid UTF-8")
    }

    /// Rotate to a new encryption key while re-encrypting data.
    ///
    /// ## Usage
    /// ```ignore
    /// let old_service = EncryptionService::new(&old_key)?;
    /// let new_service = EncryptionService::new(&new_key)?;
    ///
    /// let old_encrypted = old_service.encrypt("data")?;
    /// let new_encrypted = EncryptionService::rotate(&old_service, &new_service, &old_encrypted)?;
    /// ```
    pub fn rotate(old_service: &Self, new_service: &Self, old_ciphertext: &str) -> Result<String> {
        let plaintext = old_service.decrypt(old_ciphertext)?;
        new_service.encrypt(&plaintext)
    }
}

/// Generate a new cryptographically secure 256-bit encryption key.
///
/// ## Returns
/// Base64-encoded 32-byte random key suitable for AES-256-GCM
///
/// ## Usage
/// ```ignore
/// let key = generate_encryption_key();
/// println!("ENCRYPTION_KEY={}", key);
/// ```
pub fn generate_encryption_key() -> String {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    BASE64.encode(key)
}
