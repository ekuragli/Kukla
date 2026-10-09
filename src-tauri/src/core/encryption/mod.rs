pub mod credentials;
pub mod vault;

use anyhow::{Context, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use std::path::Path;

pub struct EncryptionService;

fn argon2_err(e: argon2::password_hash::Error) -> anyhow::Error {
    anyhow::anyhow!("Argon2 error: {}", e)
}

fn argon2id() -> Argon2<'static> {
    Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default())
}

impl EncryptionService {
    #[allow(dead_code)]
    pub fn new(_password: &str) -> Result<Self> {
        Ok(EncryptionService)
    }

    pub fn verify_password(password: impl AsRef<str>, stored_hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(stored_hash).map_err(argon2_err)?;
        Ok(argon2id()
            .verify_password(password.as_ref().as_bytes(), &parsed_hash)
            .is_ok())
    }

    pub fn hash_password(password: impl AsRef<str>) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = argon2id();
        let hash = argon2
            .hash_password(password.as_ref().as_bytes(), &salt)
            .map_err(argon2_err)?;
        Ok(hash.to_string())
    }

    #[allow(dead_code)]
    pub fn encrypt_file(input_path: &Path, output_path: &Path, passphrase: &str) -> Result<()> {
        let salt = SaltString::generate(&mut OsRng);
        let mut salt_bytes = [0u8; 16];
        let salt_str = salt.as_str();
        let bytes = salt_str.as_bytes();
        let len = bytes.len().min(16);
        salt_bytes[..len].copy_from_slice(&bytes[..len]);
        let key = Self::derive_db_key(passphrase, &salt_bytes)?;
        let cipher = ChaCha20Poly1305::new_from_slice(&key)
            .map_err(|e| anyhow::anyhow!("Failed to create cipher: {}", e))?;

        let input = std::fs::read(input_path)
            .context("Failed to read input file")?;
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, input.as_ref())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        let mut output = salt_bytes.to_vec();
        output.extend(nonce_bytes);
        output.extend(ciphertext);

        std::fs::write(output_path, &output)
            .context("Failed to write encrypted file")?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn decrypt_file(input_path: &Path, output_path: &Path, passphrase: &str) -> Result<()> {
        let data = std::fs::read(input_path)
            .context("Failed to read encrypted file")?;
        if data.len() < 28 {
            anyhow::bail!("Invalid encrypted file: too short");
        }

        let (salt_bytes, rest) = data.split_at(16);
        let (nonce_bytes, ciphertext) = rest.split_at(12);
        let mut salt = [0u8; 16];
        salt.copy_from_slice(salt_bytes);
        let key = Self::derive_db_key(passphrase, &salt)?;
        let cipher = ChaCha20Poly1305::new_from_slice(&key)
            .map_err(|e| anyhow::anyhow!("Failed to create cipher: {}", e))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

        std::fs::write(output_path, &plaintext)
            .context("Failed to write decrypted file")?;

        Ok(())
    }

    pub fn derive_db_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
        let argon2 = argon2id();
        let mut output = [0u8; 32];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut output)
            .map_err(|e| anyhow::anyhow!("Database key derivation failed: {}", e))?;
        Ok(output)
    }

    pub fn encrypt_bytes(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = ChaCha20Poly1305::new_from_slice(key)
            .map_err(|e| anyhow::anyhow!("Şifreleme anahtarı geçersiz: {}", e))?;

        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| anyhow::anyhow!("Şifreleme başarısız: {}", e))?;

        let mut output = nonce_bytes.to_vec();
        output.extend(ciphertext);
        Ok(output)
    }

    pub fn decrypt_bytes(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 12 {
            anyhow::bail!("Geçersiz şifreli veri: çok kısa");
        }

        let cipher = ChaCha20Poly1305::new_from_slice(key)
            .map_err(|e| anyhow::anyhow!("Şifre çözme anahtarı geçersiz: {}", e))?;

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Şifre çözme başarısız: {}", e))
    }
}
