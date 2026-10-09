use kukla_lib::core::audit::AuditLogger;
use kukla_lib::models::AuditEventType;
use std::path::PathBuf;

fn temp_log_path() -> PathBuf {
    std::env::temp_dir().join(format!("kukla_audit_test_{}.log", uuid::Uuid::new_v4()))
}

#[test]
fn audit_does_not_write_plaintext_before_key_set() {
    let path = temp_log_path();
    let logger = AuditLogger::new(Some(path.clone()));
    logger.log(AuditEventType::AppOpen, None);
    assert!(!path.exists(), "anahtarsız düz metin log yazılmamalı");
    let key = [5u8; 32];
    logger.set_encryption_key(key);
    let enc_path = path.with_extension("enc");
    let enc_raw = std::fs::read_to_string(&enc_path).unwrap();
    assert!(enc_raw.lines().filter(|l| !l.trim().is_empty()).count() == 1);
    let _ = std::fs::remove_file(&enc_path);
}

#[test]
fn audit_buffers_pending_until_key_set() {
    let path = temp_log_path();
    let enc_path = path.with_extension("enc");
    let logger = AuditLogger::new(Some(path.clone()));
    logger.log(AuditEventType::LoginSuccess, None);
    logger.log(AuditEventType::Search, Some("bedesten:test"));
    assert!(!path.exists(), "anahtarsız şifreli log yazılmamalı");
    let key = [5u8; 32];
    logger.set_encryption_key(key);
    let enc_raw = std::fs::read_to_string(&enc_path).unwrap();
    let line_count = enc_raw.lines().filter(|l| !l.trim().is_empty()).count();
    assert_eq!(line_count, 2, "bekleyen günlükler anahtar sonrası yazılmalı");
    let _ = std::fs::remove_file(&enc_path);
}

#[test]
fn audit_writes_encrypted_when_key_present() {
    let path = temp_log_path();
    let enc_path = path.with_extension("enc");
    let logger = AuditLogger::new(Some(path.clone()));
    logger.set_encryption_key([3u8; 32]);
    logger.log(AuditEventType::Export, Some("pdf"));
    let enc_raw = std::fs::read_to_string(&enc_path).unwrap();
    assert!(!enc_raw.contains("export"), "şifreli log düz metin içermemeli");
    assert!(enc_raw.lines().filter(|l| !l.trim().is_empty()).count() == 1);
    let _ = std::fs::remove_file(&enc_path);
}
