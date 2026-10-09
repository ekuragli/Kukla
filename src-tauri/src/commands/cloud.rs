//! Bulut sağlayıcı komutları: yapılandırma, anahtar, model listesi ve
//! sohbet arka ucu çözümlemesi.

use tauri::State;

use crate::core::cloud_llm::{preset, CloudClient, PRESETS};
use crate::models::{
    AppSettings, AiSecretsStatus, AuditEventType, CloudPresetInfo, CloudStatus,
};
use crate::AppState;

/// Sohbetin kullanacağı bulut arka ucu (yapılandırma tamamsa).
pub struct CloudBackend {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub provider: String,
}

/// Ayarlarda eksiksiz bir bulut yapılandırması varsa döner.
pub fn resolve_cloud_backend(state: &AppState) -> Result<Option<CloudBackend>, String> {
    let settings = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_settings()
        .map_err(|e| format!("Ayarlar okunamadı: {}", e))?;

    let Some(provider) = settings
        .cloud_provider
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let Some(model) = settings
        .cloud_model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let base_url = if provider == "custom" {
        settings
            .cloud_base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.trim_end_matches('/').to_string())
    } else {
        preset(provider).map(|p| p.base_url.to_string())
    };
    let Some(base_url) = base_url else {
        return Ok(None);
    };

    let api_key = cloud_api_key(state).ok_or_else(|| {
        "Bulut sağlayıcı anahtarı tanımlı değil. Ayarlar → YZ Altyapısı bölümünden ekleyin."
            .to_string()
    })?;

    Ok(Some(CloudBackend {
        base_url,
        api_key,
        model: model.to_string(),
        provider: provider.to_string(),
    }))
}

fn cloud_api_key(state: &AppState) -> Option<String> {
    state
        .db
        .read()
        .ok()
        .and_then(|db| db.get_ai_secrets().ok())
        .and_then(|secrets| secrets.cloud_api_key)
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

fn preset_infos() -> Vec<CloudPresetInfo> {
    PRESETS
        .iter()
        .map(|preset| CloudPresetInfo {
            id: preset.id.to_string(),
            label: preset.label.to_string(),
            base_url: preset.base_url.to_string(),
            hint: preset.hint.to_string(),
            needs_base_url: preset.id == "custom",
            supports_models: !preset.models_url.is_empty(),
        })
        .collect()
}

#[tauri::command]
pub async fn get_cloud_status(state: State<'_, AppState>) -> Result<CloudStatus, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let settings: AppSettings = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_settings()
        .map_err(|e| format!("Ayarlar okunamadı: {}", e))?;
    let secrets = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_ai_secrets()
        .map_err(|e| format!("Anahtarlar okunamadı: {}", e))?;

    let mut models: Vec<String> = Vec::new();
    let mut quota: Option<String> = None;

    if let Some(api_key) = secrets.cloud_api_key.as_deref() {
        if let Some(provider) = settings.cloud_provider.as_deref() {
            let base_url = if provider == "custom" {
                settings
                    .cloud_base_url
                    .clone()
                    .unwrap_or_default()
            } else {
                preset(provider).map(|p| p.base_url.to_string()).unwrap_or_default()
            };
            if !base_url.is_empty() {
                let client = CloudClient::new(&base_url, api_key, settings.cloud_model.clone().unwrap_or_default());
                if let Some(preset) = preset(provider) {
                    if let Ok(list) = client.list_models(preset.models_url).await {
                        models = list;
                    }
                    if let Ok(quota_text) = client.quota(preset.quota_url.unwrap_or("")).await {
                        quota = quota_text;
                    }
                }
            }
        }
    }

    let backend_ready = resolve_cloud_backend(&state)?.is_some();
    Ok(CloudStatus {
        provider: settings.cloud_provider,
        model: settings.cloud_model,
        base_url: settings.cloud_base_url,
        has_key: secrets.cloud_api_key.is_some(),
        key_hint: secrets
            .cloud_api_key
            .as_deref()
            .map(crate::models::mask_key),
        models,
        quota,
        chat_backend: if backend_ready { "cloud".to_string() } else { "opencode".to_string() },
        presets: preset_infos(),
    })
}

/// Sağlayıcı/model/adres yapılandırmasını günceller (anahtar hariç).
#[tauri::command]
pub async fn set_cloud_config(
    state: State<'_, AppState>,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
) -> Result<CloudStatus, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let mut settings = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_settings()
        .map_err(|e| format!("Ayarlar okunamadı: {}", e))?;

    settings.cloud_provider = provider
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    settings.cloud_model = model
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    settings.cloud_base_url = base_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    // Sağlayıcı değiştiğinde opencode oturumlarını korumak için opencode modeli
    // değiştirilmez; sohbet oturumu kendi backend'ini modelde tutar.
    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .update_settings(&settings)
        .map_err(|e| format!("Ayarlar güncellenemedi: {}", e))?;

    state
        .audit
        .log(AuditEventType::SettingsChange, Some("cloud_config"));

    get_cloud_status(state).await
}

/// Bulut sağlayıcı anahtarını kaydeder/siler.
#[tauri::command]
pub async fn set_cloud_key(
    state: State<'_, AppState>,
    key: Option<String>,
) -> Result<AiSecretsStatus, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .set_cloud_api_key(key.as_deref())
        .map_err(|e| format!("Anahtar kaydedilemedi: {}", e))?;

    state
        .audit
        .log(AuditEventType::SettingsChange, Some("cloud_key"));

    let secrets = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_ai_secrets()
        .map_err(|e| format!("Anahtar durumu okunamadı: {}", e))?;
    Ok(AiSecretsStatus::from_secrets(&secrets))
}
