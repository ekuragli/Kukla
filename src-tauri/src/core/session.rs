//! Oturum verilerinin (SQLite + vektör deposu) şifrelenme akışı.
//!
//! Veriler oturum açıkken düz metin olarak diskte tutulur; kilit, çıkış ve
//! şifre değişiminde bu fonksiyonlarla şifrelenir. Windows'ta açık SQLite
//! dosyası silinemediği için önce bağlantılar bellekteki kopyalara çevrilir.

use std::path::Path;
use std::sync::RwLock;

use crate::core::encryption::vault;
use crate::db::sqlite::MetadataDb;
use crate::db::vector_store::VectorDb;

/// Şifreli veri dosyalarının kilidini açar ve bağlantıları kurar.
pub fn open_data_stores(data_dir: &Path, key: &[u8; 32]) -> Result<(MetadataDb, VectorDb), String> {
    vault::unlock_data_files(data_dir, key)
        .map_err(|e| format!("Verilerin kilidi açılamadı: {}", e))?;

    let db_path = data_dir.join("kukla.db");
    let db =
        MetadataDb::connect(&db_path).map_err(|e| format!("Veritabanı açılamadı: {}", e))?;

    let vector_db = VectorDb::connect(data_dir).unwrap_or_else(|_| VectorDb::new_in_memory());

    Ok((db, vector_db))
}

/// Veri dosyalarını verilen anahtarla şifreler ve dosya tutamaçlarını kapatır.
///
/// Şifreleme başarısız olursa düz metin dosyalar yerinde kaldığı için oturum
/// aynı anahtarla yeniden açılır.
pub fn seal_data_stores(
    db: &RwLock<MetadataDb>,
    vector_db: &RwLock<VectorDb>,
    data_dir: &Path,
    key: &[u8; 32],
) -> Result<(), String> {
    if let Ok(db) = db.read() {
        let _ = db.checkpoint();
    }
    if let Ok(vector_db) = vector_db.read() {
        let _ = vector_db.save();
    }

    let in_memory_db = MetadataDb::connect_memory()
        .map_err(|e| format!("Veritabanı sıfırlanamadı: {}", e))?;
    let in_memory_vector = VectorDb::new_in_memory();

    {
        let mut guard = db.write().map_err(|_| "Veritabanı kilitli.".to_string())?;
        let previous = std::mem::replace(&mut *guard, in_memory_db);
        drop(previous);
    }
    {
        let mut guard = vector_db
            .write()
            .map_err(|_| "Vector store kilitli.".to_string())?;
        let previous = std::mem::replace(&mut *guard, in_memory_vector);
        drop(previous);
    }

    if let Err(error) = vault::seal_data_files(data_dir, key) {
        if let Ok((restored_db, restored_vector)) = open_data_stores(data_dir, key) {
            if let Ok(mut guard) = db.write() {
                *guard = restored_db;
            }
            if let Ok(mut guard) = vector_db.write() {
                *guard = restored_vector;
            }
        }
        return Err(format!("Veriler şifrelenemedi: {}", error));
    }

    Ok(())
}

/// Şifre değişiminde verileri yeni anahtarla yeniden şifreler ve oturumu sürdürür.
///
/// Şifre değiştirilip yalnızca bellek içi anahtar güncellenirse diskteki şifreli
/// kopyalar eski anahtarla kalır ve yeni şifreyle açılamaz.
pub fn rekey_data_stores(
    db: &RwLock<MetadataDb>,
    vector_db: &RwLock<VectorDb>,
    data_dir: &Path,
    new_key: &[u8; 32],
) -> Result<(), String> {
    seal_data_stores(db, vector_db, data_dir, new_key)?;

    let (reopened_db, reopened_vector) = open_data_stores(data_dir, new_key)?;
    if let Ok(mut guard) = db.write() {
        *guard = reopened_db;
    }
    if let Ok(mut guard) = vector_db.write() {
        *guard = reopened_vector;
    }

    Ok(())
}
