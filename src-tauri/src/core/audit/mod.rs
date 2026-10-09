use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chrono::{Duration, Utc};
use serde_json::json;
use std::io::Write;
use std::sync::Mutex;
use tracing::{error, info, warn};
use zeroize::Zeroizing;

use crate::core::encryption::EncryptionService;
use crate::models::AuditEventType;

pub struct AuditLogger {
    log_path: Option<std::path::PathBuf>,
    encryption_key: Mutex<Option<Zeroizing<[u8; 32]>>>,
    pending: Mutex<Vec<String>>,
}

impl AuditLogger {
    pub fn new(log_path: Option<std::path::PathBuf>) -> Self {
        AuditLogger {
            log_path,
            encryption_key: Mutex::new(None),
            pending: Mutex::new(Vec::new()),
        }
    }

    pub fn set_encryption_key(&self, key: [u8; 32]) {
        if let Ok(mut guard) = self.encryption_key.lock() {
            *guard = Some(Zeroizing::new(key));
        }
        self.flush_pending();
    }

    fn flush_pending(&self) {
        let buffered = {
            let mut pending = match self.pending.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            std::mem::take(&mut *pending)
        };
        for line in buffered {
            self.write_log_line(&line);
        }
    }

    pub fn log(&self, event_type: AuditEventType, details: Option<&str>) {
        let timestamp = Utc::now();
        let event_name = event_type.as_str();
        let log_entry = json!({
            "timestamp": timestamp.to_rfc3339(),
            "event": event_name,
            "details": details,
        });

        let log_line = log_entry.to_string();

        match event_type {
            AuditEventType::LoginFail => warn!("{}", log_line),
            AuditEventType::PersonalArchiveAdd | AuditEventType::PersonalArchiveDelete => {
                info!("{}", log_line)
            }
            AuditEventType::Export => info!("{}", log_line),
            _ => info!("{}", log_line),
        }

        self.write_log_line(&log_line);
    }

    fn encrypted_log_path(path: &std::path::Path) -> std::path::PathBuf {
        path.with_extension("enc")
    }

    fn write_log_line(&self, log_line: &str) {
        let Some(ref path) = self.log_path else {
            return;
        };

        let key_guard = match self.encryption_key.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if let Some(ref key) = *key_guard {
            let enc_path = Self::encrypted_log_path(path);
            match EncryptionService::encrypt_bytes(key, log_line.as_bytes()) {
                Ok(encrypted) => {
                    let encoded = B64.encode(encrypted);
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&enc_path)
                    {
                        let _ = writeln!(file, "{encoded}");
                        return;
                    }
                }
                Err(e) => error!("Audit log encryption failed: {}", e),
            }
        } else {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.push(log_line.to_string());
        }
    }

    fn read_log_lines(path: &std::path::Path, key: Option<&[u8; 32]>) -> Vec<String> {
        let enc_path = Self::encrypted_log_path(path);
        if enc_path.exists() {
            if let Some(key) = key {
                if let Ok(file) = std::fs::read_to_string(&enc_path) {
                    let mut lines = Vec::new();
                    for line in file.lines() {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Ok(bytes) = B64.decode(trimmed) {
                            if let Ok(plain) = EncryptionService::decrypt_bytes(key, &bytes) {
                                if let Ok(text) = String::from_utf8(plain) {
                                    lines.push(text);
                                }
                            }
                        }
                    }
                    if !lines.is_empty() {
                        return lines;
                    }
                }
            }
        }

        if let Ok(file) = std::fs::read_to_string(path) {
            return file.lines().map(|l| l.to_string()).collect();
        }

        Vec::new()
    }

    pub fn log_app_open(&self) {
        self.log(AuditEventType::AppOpen, None);
    }

    pub fn log_login_success(&self) {
        self.log(AuditEventType::LoginSuccess, None);
    }

    pub fn log_app_lock(&self) {
        self.log(AuditEventType::AppLock, None);
    }

    pub fn log_login_fail(&self, attempt: u32) {
        self.log(
            AuditEventType::LoginFail,
            Some(&format!("failed_attempt_{}", attempt)),
        );
    }

    pub fn log_search(&self, query: &str) {
        self.log(AuditEventType::Search, Some(query));
    }

    pub fn log_export(&self, format: &str) {
        self.log(AuditEventType::Export, Some(format));
    }

    pub fn log_personal_archive_add(&self, filename: &str) {
        self.log(AuditEventType::PersonalArchiveAdd, Some(filename));
    }

    pub fn log_personal_archive_delete(&self, decision_id: i64) {
        self.log(
            AuditEventType::PersonalArchiveDelete,
            Some(&format!("decision_{}", decision_id)),
        );
    }

    pub fn cleanup_old_logs(&self, retention_days: u32) {
        if retention_days == 0 {
            return;
        }
        let cutoff = Utc::now() - Duration::days(retention_days as i64);
        let Some(ref path) = self.log_path else {
            return;
        };

        let key_copy: Option<[u8; 32]> = self.encryption_key.lock().ok().and_then(|g| {
            g.as_ref().map(|k| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(k.as_ref());
                arr
            })
        });

        let key_ref = key_copy.as_ref();
        let lines = Self::read_log_lines(path, key_ref);

        if lines.is_empty() {
            return;
        }

        let mut keep = Vec::new();
        let mut removed = 0u32;
        for line in lines {
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) {
                let timestamp = entry["timestamp"].as_str().unwrap_or("");
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp) {
                    if ts.to_utc() >= cutoff {
                        keep.push(line);
                    } else {
                        removed += 1;
                    }
                } else {
                    keep.push(line);
                }
            } else {
                keep.push(line);
            }
        }

        if removed == 0 {
            return;
        }

        if let Some(key) = key_copy.as_ref() {
            let enc_path = Self::encrypted_log_path(path);
            if let Ok(mut file) = std::fs::File::create(&enc_path) {
                for line in keep {
                    if let Ok(encrypted) = EncryptionService::encrypt_bytes(key, line.as_bytes())
                    {
                        let encoded = B64.encode(encrypted);
                        let _ = writeln!(file, "{encoded}");
                    }
                }
                info!(
                    "Cleaned up {} old encrypted audit log entries (>{}.gün)",
                    removed, retention_days
                );
            }
        } else if let Ok(mut file) = std::fs::File::create(path) {
            for line in keep {
                let _ = writeln!(file, "{line}");
            }
            info!(
                "Cleaned up {} old audit log entries (>{}.gün)",
                removed, retention_days
            );
        }
    }
}