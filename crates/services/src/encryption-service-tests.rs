#[cfg(test)]
mod tests {
    use super::super::encryption_service::*;
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

    fn key_from_byte(byte: u8) -> String {
        BASE64.encode([byte; 32])
    }

    fn test_service() -> EncryptionService {
        // Test key (32 bytes base64 encoded)
        EncryptionService::new(&key_from_byte(0)).unwrap()
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let service = test_service();
        let plaintext = "my-super-secret-gitlab-pat-12345";

        let encrypted = service.encrypt(plaintext).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertexts() {
        let service = test_service();
        let plaintext = "same-data";

        let encrypted1 = service.encrypt(plaintext).unwrap();
        let encrypted2 = service.encrypt(plaintext).unwrap();

        // Different nonces should produce different ciphertexts
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to same plaintext
        assert_eq!(service.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(service.decrypt(&encrypted2).unwrap(), plaintext);
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        let service = test_service();
        let result = service.decrypt("not-valid-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_too_short() {
        let service = test_service();
        let short_data = BASE64.encode([1, 2, 3]); // Less than 12 bytes
        let result = service.decrypt(&short_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_corrupted_ciphertext() {
        let service = test_service();
        let encrypted = service.encrypt("data").unwrap();

        // Corrupt the ciphertext
        let mut bytes = BASE64.decode(&encrypted).unwrap();
        bytes[15] ^= 0xFF; // Flip bits
        let corrupted = BASE64.encode(bytes);

        let result = service.decrypt(&corrupted);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_key_fails_decryption() {
        let service1 = EncryptionService::new(&key_from_byte(0)).unwrap();
        let service2 = EncryptionService::new(&key_from_byte(1)).unwrap();

        let encrypted = service1.encrypt("secret").unwrap();
        let result = service2.decrypt(&encrypted);

        assert!(result.is_err());
    }

    #[test]
    fn test_key_rotation() {
        let old_service = EncryptionService::new(&key_from_byte(0)).unwrap();
        let new_service = EncryptionService::new(&key_from_byte(1)).unwrap();

        let plaintext = "rotate-this-secret";
        let old_encrypted = old_service.encrypt(plaintext).unwrap();

        let new_encrypted =
            EncryptionService::rotate(&old_service, &new_service, &old_encrypted).unwrap();

        // Old service can't decrypt new ciphertext
        assert!(old_service.decrypt(&new_encrypted).is_err());

        // New service can decrypt new ciphertext
        assert_eq!(new_service.decrypt(&new_encrypted).unwrap(), plaintext);
    }

    #[test]
    fn test_generate_key() {
        let key = generate_encryption_key();

        // Should be valid base64
        let decoded = BASE64.decode(&key).unwrap();

        // Should be 32 bytes
        assert_eq!(decoded.len(), 32);

        // Should create valid service
        let service = EncryptionService::new(&key).unwrap();
        assert!(service.encrypt("test").is_ok());
    }

    #[test]
    fn test_invalid_key_length() {
        let short_key = BASE64.encode([1, 2, 3]); // Too short
        let result = EncryptionService::new(&short_key);
        assert!(result.is_err());
    }
}
