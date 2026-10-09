use kukla_lib::core::encryption::vault;
use kukla_lib::core::session::{open_data_stores, rekey_data_stores, seal_data_stores};
use kukla_lib::db::sqlite::MetadataDb;
use kukla_lib::db::vector_store::VectorDb;
use kukla_lib::models::{DecisionSource, NewCase, NewDecision};
use std::sync::RwLock;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kukla_{}_{}", tag, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Oturum açıkken şifre değiştirmek veriyi korumalı ve yeni anahtarla
/// şifrelenmeli; eski anahtarla tekrar açılamamalı.
#[test]
fn password_change_rekeys_data_without_data_loss() {
    let dir = temp_dir("session_rekey");

    // Oturum açılışı: düz metin dosyalar üzerinde çalışan bağlantılar.
    let db = MetadataDb::connect(&dir.join("kukla.db")).unwrap();
    let vector_db = VectorDb::connect(&dir).unwrap();
    let case_id = db
        .insert_case(&NewCase {
            query_text: "kira uyarlama".to_string(),
            pdf_path: None,
            pdf_hash: None,
        })
        .unwrap();
    let decision_id = db
        .insert_decision(&NewDecision {
            case_id,
            source: DecisionSource::Yargitay,
            esas_no: Some("2026/100".to_string()),
            karar_no: Some("5000".to_string()),
            karar_tarihi: None,
            daire: Some("9. Hukuk".to_string()),
            summary: None,
            ratio_decidendi: None,
            similarity_score: Some(0.9),
        })
        .unwrap();

    let db = RwLock::new(db);
    let vector_db = RwLock::new(vector_db);

    // Şifre değişimi: veriler yeni anahtarla yeniden şifrelenir, oturum sürer.
    let old_key = [11u8; 32];
    let new_key = [22u8; 32];
    rekey_data_stores(&db, &vector_db, &dir, &new_key).unwrap();

    let stored = db
        .read()
        .unwrap()
        .get_decision(decision_id)
        .unwrap()
        .expect("karar yeniden şifreleme sonrası kaybolmamalı");
    assert_eq!(stored.karar_no.as_deref(), Some("5000"));

    // Kilit: düz metin dosyalar diskten silinir.
    seal_data_stores(&db, &vector_db, &dir, &new_key).unwrap();
    assert!(!dir.join("kukla.db").exists());
    assert!(!dir.join("vectors.bin").exists());
    assert!(dir.join("kukla.db.enc").exists());
    assert!(dir.join("vectors.bin.enc").exists());

    // Eski anahtarla açılamaz.
    assert!(vault::unlock_data_files(&dir, &old_key).is_err());

    // Yeni anahtarla açılır ve veri geri gelir.
    let (reopened_db, _reopened_vector) = open_data_stores(&dir, &new_key).unwrap();
    let stored = reopened_db
        .get_decision(decision_id)
        .unwrap()
        .expect("karar yeni anahtarla açılmalı");
    assert_eq!(stored.esas_no.as_deref(), Some("2026/100"));

    let _ = std::fs::remove_dir_all(&dir);
}

/// Kilit/çıkış akışı açık bağlantılara rağmen düz metin dosyaları siler.
#[test]
fn seal_removes_plaintext_while_connections_released() {
    let dir = temp_dir("session_seal");
    let db = RwLock::new(MetadataDb::connect(&dir.join("kukla.db")).unwrap());
    let vector_db = RwLock::new(VectorDb::connect(&dir).unwrap());

    let key = [33u8; 32];
    seal_data_stores(&db, &vector_db, &dir, &key).unwrap();

    assert!(!dir.join("kukla.db").exists());
    assert!(!dir.join("vectors.bin").exists());
    assert!(dir.join("kukla.db.enc").exists());
    assert!(!dir.join("kukla.db-wal").exists());
    assert!(!dir.join("kukla.db-shm").exists());

    let _ = std::fs::remove_dir_all(&dir);
}
