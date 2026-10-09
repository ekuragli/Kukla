use tauri::State;

use crate::models::AppSettings;
use crate::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_settings()
        .map_err(|e| format!("Ayarlar alınamadı: {}", e))
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let previous = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_settings()
        .map_err(|e| format!("Ayarlar okunamadı: {}", e))?;

    let mut settings = settings;

    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .update_settings(&settings)
        .map_err(|e| format!("Ayarlar güncellenemedi: {}", e))?;

    let models_changed = previous.language != settings.language
        || previous.auto_lock_minutes != settings.auto_lock_minutes
        || previous.log_retention_days != settings.log_retention_days
        || previous.personal_archive_enabled != settings.personal_archive_enabled
        || previous.ai_model != settings.ai_model;

    if models_changed {
    }

    state
        .audit
        .log(crate::models::AuditEventType::SettingsChange, Some("settings_updated"));
    Ok(())
}