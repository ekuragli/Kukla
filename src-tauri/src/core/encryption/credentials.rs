use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCredentials {
    pub password_hash: String,
    pub encryption_salt: String,
}

impl AuthCredentials {
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("auth.json")
    }

    pub fn load(data_dir: &Path) -> Result<Option<Self>> {
        let path = Self::path(data_dir);
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read_to_string(&path).context("auth.json okunamadı")?;
        let creds: AuthCredentials =
            serde_json::from_str(&data).context("auth.json ayrıştırılamadı")?;
        Ok(Some(creds))
    }

    pub fn save(&self, data_dir: &Path) -> Result<()> {
        let path = Self::path(data_dir);
        std::fs::write(&path, serde_json::to_string(self)?).context("auth.json yazılamadı")?;
        Ok(())
    }

    pub fn create_new(password_hash: &str) -> Self {
        let salt: [u8; 16] = rand::random();
        AuthCredentials {
            password_hash: password_hash.to_string(),
            encryption_salt: STANDARD.encode(salt),
        }
    }

    pub fn salt_bytes(&self) -> Result<[u8; 16]> {
        let decoded = STANDARD
            .decode(&self.encryption_salt)
            .context("Şifreleme tuzu çözümlenemedi")?;
        if decoded.len() != 16 {
            anyhow::bail!("Geçersiz şifreleme tuzu uzunluğu");
        }
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&decoded);
        Ok(salt)
    }
}

pub fn migrate_auth_from_db(data_dir: &Path) -> Result<()> {
    if AuthCredentials::load(data_dir)?.is_some() {
        return Ok(());
    }

    let db_path = data_dir.join("kukla.db");
    if !db_path.exists() {
        return Ok(());
    }

    let db = crate::db::sqlite::MetadataDb::connect(&db_path)?;
    if let Some(hash) = db.get_password_hash()? {
        let creds = AuthCredentials::create_new(&hash);
        creds.save(data_dir)?;
        tracing::info!("Şifre hash'i auth.json dosyasına taşındı");
    }

    Ok(())
}