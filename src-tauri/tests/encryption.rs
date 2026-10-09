use kukla_lib::core::encryption::EncryptionService;
use kukla_lib::core::encryption::vault::{seal_file, unseal_file};
use std::fs;

#[test]
fn encrypt_decrypt_roundtrip() {
    let key = [7u8; 32];
    let plain = b"kukla test verisi";
    let encrypted = EncryptionService::encrypt_bytes(&key, plain).unwrap();
    let decrypted = EncryptionService::decrypt_bytes(&key, &encrypted).unwrap();
    assert_eq!(decrypted, plain);
}

#[test]
fn seal_and_unseal_roundtrip() {
    let dir = std::env::temp_dir().join(format!("kukla_vault_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    let plain = dir.join("test.bin");
    let enc = dir.join("test.bin.enc");
    fs::write(&plain, b"kukla secret data").unwrap();

    let key = [9u8; 32];
    seal_file(&plain, &enc, &key).unwrap();
    assert!(!plain.exists());
    assert!(enc.exists());

    unseal_file(&enc, &plain, &key).unwrap();
    let content = fs::read(&plain).unwrap();
    assert_eq!(content, b"kukla secret data");

    let _ = fs::remove_dir_all(&dir);
}