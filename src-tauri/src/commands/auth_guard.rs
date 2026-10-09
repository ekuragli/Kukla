use std::time::{Duration, Instant};

use crate::AppState;

const MUTEX_ERR: &str = "Kimlik doğrulama durumu okunamadı.";
const LOCKED_MSG: &str = "Oturum kilitli. Lütfen şifrenizi girin.";

pub fn locked_session_message(is_authenticated: bool) -> Option<&'static str> {
    if is_authenticated {
        None
    } else {
        Some(LOCKED_MSG)
    }
}

pub fn validate_password_strength(password: &str) -> Result<(), String> {
    if password.len() < 12 {
        return Err("Şifre en az 12 karakter olmalıdır.".to_string());
    }

    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    if !(has_upper && has_lower && has_digit && has_special) {
        return Err(
            "Şifre büyük/küçük harf, rakam ve özel karakter içermelidir.".to_string(),
        );
    }

    Ok(())
}

pub fn session_timed_out(last_activity: Instant, auto_lock_minutes: u32) -> bool {
    if auto_lock_minutes == 0 {
        return false;
    }
    let timeout = Duration::from_secs(auto_lock_minutes as u64 * 60);
    last_activity.elapsed() > timeout
}

/// Oturum verilerini şifreler ve oturum durumunu sıfırlar.
///
/// Dosya tutamaçları önce kapatılır (Windows'ta açık SQLite dosyası silinemez),
/// ardından düz metin dosyalar silinir.
pub fn seal_session_data(state: &AppState) -> Result<(), String> {
    let key = {
        let key_guard = state.db_key.lock().map_err(|_| MUTEX_ERR)?;
        key_guard.clone()
    };
    let Some(key) = key else {
        return Ok(());
    };

    state.seal_with_key(&key)?;

    *state.db_key.lock().map_err(|_| MUTEX_ERR)? = None;

    Ok(())
}

fn lock_session(state: &AppState, reason: &str) {
    let _ = seal_session_data(state);
    if let Ok(mut auth) = state.is_authenticated.lock() {
        *auth = false;
    }
    state
        .audit
        .log(crate::models::AuditEventType::AppLock, Some(reason));
}

pub fn check_session(state: &AppState) -> Result<(), String> {
    let is_auth = *state
        .is_authenticated
        .lock()
        .map_err(|_| MUTEX_ERR)?;

    if let Some(msg) = locked_session_message(is_auth) {
        return Err(msg.to_string());
    }

    let settings = state
        .db
        .read()
        .ok()
        .and_then(|db| db.get_settings().ok())
        .unwrap_or_default();
    let auto_lock_mins = settings.auto_lock_minutes;

    if auto_lock_mins > 0 {
        let last = *state
            .last_activity
            .lock()
            .map_err(|_| MUTEX_ERR)?;
        if session_timed_out(last, auto_lock_mins) {
            lock_session(state, "auto_lock");
            return Err(
                "Oturum zaman aşımı nedeniyle kilitlendi. Lütfen şifrenizi girin.".to_string(),
            );
        }
    }

    Ok(())
}

pub fn require_auth(state: &AppState) -> Result<(), String> {
    check_session(state)?;
    if let Ok(mut last) = state.last_activity.lock() {
        *last = Instant::now();
    }
    Ok(())
}

pub fn touch_activity(state: &AppState) -> Result<(), String> {
    check_session(state)?;
    if let Ok(mut last) = state.last_activity.lock() {
        *last = Instant::now();
    }
    Ok(())
}

pub fn reset_activity(state: &AppState) {
    if let Ok(mut last) = state.last_activity.lock() {
        *last = Instant::now();
    }
}

const LOCKOUT_FILE: &str = "lockout_until.json";

pub fn persist_lockout(state: &AppState, until: std::time::Instant) {
    let path = state.data_dir.join(LOCKOUT_FILE);
    let now = std::time::SystemTime::now();
    let remaining = until.saturating_duration_since(Instant::now());
    let target = now + remaining;
    if let Ok(secs) = target.duration_since(std::time::UNIX_EPOCH) {
        let _ = std::fs::write(&path, secs.as_secs().to_string());
    }
}

pub fn load_persisted_lockout(state: &AppState) -> Option<Instant> {
    let path = state.data_dir.join(LOCKOUT_FILE);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return None;
    };
    let Ok(epoch_secs) = content.trim().parse::<u64>() else {
        return None;
    };
    let target = std::time::UNIX_EPOCH + std::time::Duration::from_secs(epoch_secs);
    let now = std::time::SystemTime::now();
    if target <= now {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    let remaining = target.duration_since(now).unwrap_or_default();
    Some(Instant::now() + remaining)
}

pub fn clear_persisted_lockout(state: &AppState) {
    let _ = std::fs::remove_file(state.data_dir.join(LOCKOUT_FILE));
}

