use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::EncryptionService;

/// Uygulamanın veri dosyaları: (düz metin, şifreli).
fn data_file_pairs(data_dir: &Path) -> [(PathBuf, PathBuf); 2] {
    [
        (data_dir.join("kukla.db"), data_dir.join("kukla.db.enc")),
        (
            data_dir.join("vectors.bin"),
            data_dir.join("vectors.bin.enc"),
        ),
    ]
}

pub fn derive_storage_key(password: &str, salt: &[u8; 16]) -> Result<[u8; 32]> {
    EncryptionService::derive_db_key(password, salt)
}

pub fn seal_file(plain_path: &Path, enc_path: &Path, key: &[u8; 32]) -> Result<()> {
    if !plain_path.exists() {
        return Ok(());
    }

    let data = std::fs::read(plain_path).with_context(|| {
        format!("Dosya okunamadı: {}", plain_path.display())
    })?;
    let encrypted = EncryptionService::encrypt_bytes(key, &data)?;
    write_atomic(enc_path, &encrypted).with_context(|| {
        format!("Şifreli dosya yazılamadı: {}", enc_path.display())
    })?;
    std::fs::remove_file(plain_path).ok();
    remove_sqlite_sidecars(plain_path);
    Ok(())
}

pub fn unseal_file(enc_path: &Path, plain_path: &Path, key: &[u8; 32]) -> Result<()> {
    let data = std::fs::read(enc_path).with_context(|| {
        format!("Şifreli dosya okunamadı: {}", enc_path.display())
    })?;
    let plain = EncryptionService::decrypt_bytes(key, &data)?;
    std::fs::write(plain_path, plain).with_context(|| {
        format!("Düz dosya yazılamadı: {}", plain_path.display())
    })?;
    Ok(())
}

pub fn unlock_data_files(data_dir: &Path, key: &[u8; 32]) -> Result<()> {
    for (plain, enc) in data_file_pairs(data_dir) {
        if enc.exists() {
            unseal_file(&enc, &plain, key)?;
        }
    }

    Ok(())
}

/// Tüm veri dosyalarını verilen anahtarla şifreler.
///
/// İki aşamalı çalışır: önce tüm dosyalar şifrelenip geçici dosyalara yazılır,
/// ardından birlikte yerine konur. Kısmi bir yazma hatası şifreli kopyaları
/// tutarsız bırakmaz. Düz metin dosyalar yalnızca şifreli kopyalar kalıcı
/// olduktan sonra silinir — çağıran dosya tutamaçlarını (SQLite bağlantısı vb.)
/// önce kapatmalıdır; Windows'ta açık dosya silinemez.
pub fn seal_data_files(data_dir: &Path, key: &[u8; 32]) -> Result<()> {
    let mut prepared: Vec<(PathBuf, PathBuf, Vec<u8>)> = Vec::new();

    for (plain, enc) in data_file_pairs(data_dir) {
        if !plain.exists() {
            continue;
        }

        let data = std::fs::read(&plain).with_context(|| {
            format!("Dosya okunamadı: {}", plain.display())
        })?;
        let encrypted = EncryptionService::encrypt_bytes(key, &data)?;
        prepared.push((plain, enc, encrypted));
    }

    for (plain, enc, encrypted) in &prepared {
        write_atomic(enc, encrypted).with_context(|| {
            format!("Şifreli dosya yazılamadı: {} (kaynak: {})", enc.display(), plain.display())
        })?;
    }

    for (plain, _enc, _) in &prepared {
        std::fs::remove_file(plain).ok();
        remove_sqlite_sidecars(plain);
    }

    Ok(())
}

/// Şifreli dosyayı geçici ad üzerinden yazıp yerine koyar (kısmi yazma görünmez).
fn write_atomic(enc_path: &Path, encrypted: &[u8]) -> Result<()> {
    let temp = PathBuf::from(format!("{}.tmp", enc_path.display()));
    std::fs::write(&temp, encrypted)?;
    std::fs::rename(&temp, enc_path)?;
    Ok(())
}

fn remove_sqlite_sidecars(db_path: &Path) {
    let stem = db_path.to_string_lossy();
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = PathBuf::from(format!("{}{}", stem, suffix));
        std::fs::remove_file(sidecar).ok();
    }
}
