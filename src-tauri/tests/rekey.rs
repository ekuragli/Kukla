use kukla_lib::core::encryption::vault::{seal_data_files, unlock_data_files};
use std::fs;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kukla_{}_{}", tag, uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn resealing_with_new_key_blocks_old_key() {
    let dir = temp_dir("rekey");
    fs::write(dir.join("kukla.db"), b"db-verisi").unwrap();
    fs::write(dir.join("vectors.bin"), b"vektor-verisi").unwrap();

    let old_key = [1u8; 32];
    seal_data_files(&dir, &old_key).unwrap();
    assert!(!dir.join("kukla.db").exists());
    assert!(!dir.join("vectors.bin").exists());
    assert!(dir.join("kukla.db.enc").exists());
    assert!(dir.join("vectors.bin.enc").exists());

    // Şifre değişimi: eski anahtarla aç, yeni anahtarla yeniden mühürle.
    unlock_data_files(&dir, &old_key).unwrap();
    assert_eq!(fs::read(dir.join("kukla.db")).unwrap(), b"db-verisi");
    assert_eq!(fs::read(dir.join("vectors.bin")).unwrap(), b"vektor-verisi");

    let new_key = [2u8; 32];
    seal_data_files(&dir, &new_key).unwrap();

    // Eski anahtarla artık açılamaz.
    assert!(unlock_data_files(&dir, &old_key).is_err());

    // Yeni anahtarla açılır ve içerik korunur.
    unlock_data_files(&dir, &new_key).unwrap();
    assert_eq!(fs::read(dir.join("kukla.db")).unwrap(), b"db-verisi");
    assert_eq!(fs::read(dir.join("vectors.bin")).unwrap(), b"vektor-verisi");

    // Geçici dosya bırakılmamalı.
    assert!(!dir.join("kukla.db.enc.tmp").exists());
    assert!(!dir.join("vectors.bin.enc.tmp").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn seal_only_seals_existing_files() {
    let dir = temp_dir("rekey_partial");
    fs::write(dir.join("kukla.db"), b"db-verisi").unwrap();

    seal_data_files(&dir, &[3u8; 32]).unwrap();
    assert!(dir.join("kukla.db.enc").exists());
    assert!(!dir.join("vectors.bin").exists());
    assert!(!dir.join("vectors.bin.enc").exists());

    let _ = fs::remove_dir_all(&dir);
}
