use std::sync::LazyLock;
use tauri::{Emitter, State};

use crate::core::opencode_client::{ChatReply, OpencodeClient};
use crate::models::AiStatus;
use crate::AppState;

/// opencode sunucusunun adresi. Önce çalışma zamanında bulunan adres (port
/// kaydırmasında), sonra `KUKLA_OPENCODE_URL`, en sonda varsayılan 4096.
///
/// Sunucuya erişilebildiğinden emin değilse önce `ensure_server` çağrılmalıdır.
pub fn client() -> OpencodeClient {
    OpencodeClient::from_setting(&crate::core::opencode_server::resolved_url())
}

/// Doğrudan sunucuyu çalıştırır (kullanıcının kendi sunucusu varsa ona dokunmaz).
#[tauri::command]
pub async fn start_opencode_server(state: State<'_, AppState>) -> Result<String, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    let url = crate::core::opencode_server::ensure_server().await?;
    state
        .audit
        .log(crate::models::AuditEventType::SettingsChange, Some("opencode_server_start"));
    Ok(url)
}

/// opencode + embedding altyapısının durumu.
#[tauri::command]
pub async fn get_ai_status(state: State<'_, AppState>) -> Result<AiStatus, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    Ok(ai_status(&state).await)
}

pub async fn ai_status(state: &AppState) -> AiStatus {
    let client = client();
    let mut opencode = client.status().await;
    opencode.managed_by_app = crate::core::opencode_server::is_managed();
    let embedding = crate::commands::embedding::embedding_status(state);
    let models = if opencode.healthy {
        client.list_models().await
    } else {
        Vec::new()
    };
    let selected_model = selected_model(state);
    AiStatus {
        opencode,
        embedding,
        models,
        selected_model,
    }
}

/// Ayarlar'da seçilen opencode modeli (`providerID/modelID`); yoksa `None`.
pub fn selected_model(state: &AppState) -> Option<String> {
    state
        .db
        .read()
        .ok()
        .and_then(|db| db.get_settings().ok())
        .and_then(|settings| settings.ai_model)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Kullanıcının iptal ettiği oturum kimlikleri: yoklama döngüsü bu sete bakıp
/// hemen durur.
static CANCELLED_SESSIONS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashSet<String>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

/// Ayarlar'da model seçilmediyse veya seçilen model hata verirse denenilen
/// yedekler. Ölçüm: space-bunny ~10 sn, big-pickle ~12 sn özet üretimi;
/// `exo-free` 503, `mimo-v2.6` tek istekte 100 sn'ye kadar çıkıyor.
const FALLBACK_MODELS: &[&str] = &[
    "opencode/space-bunny-free",
    "opencode/big-pickle",
    "opencode/mimo-v2.6-flash-free",
];

/// Ayarlar'da model seçilmediyse sunucunun kendi listesinden ilk model.
///
/// Yedek model listesine düşmek yerine kullanıcının sunucusundaki gerçek bir
/// model seçilir; böylece "model bulunamadı" hataları azalır.
async fn first_server_model() -> Option<String> {
    static CACHE: LazyLock<std::sync::RwLock<Option<Option<String>>>> =
        LazyLock::new(|| std::sync::RwLock::new(None));
    if let Ok(guard) = CACHE.read() {
        if let Some(cached) = guard.as_ref() {
            return cached.clone();
        }
    }
    let model = client().list_models().await.into_iter().find(|m| m.enabled).map(|m| m.label());
    if let Ok(mut guard) = CACHE.write() {
        *guard = Some(model.clone());
    }
    model
}

/// Denenecek model sırası: Ayarlar'daki seçim + sunucunun ilk modeli +
/// yedekler (en fazla 2 deneme, ikinci deneme gecikmeyi kabul edilebilir tutar).
async fn model_chain(state: &AppState) -> Vec<String> {
    let mut chain: Vec<String> = Vec::new();
    if let Some(model) = selected_model(state) {
        chain.push(model);
    } else {
        match first_server_model().await {
            Some(model) => {
                tracing::info!("Ayarlar'da model seçilmedi; sunucunun ilk modeli kullanılıyor: {}", model);
                chain.push(model);
            }
            None => {
                tracing::warn!(
                    "Ayarlar'da model seçilmedi ve sunucudan model alınamadı; hızlı yedek kullanılıyor: {}",
                    FALLBACK_MODELS[0]
                );
            }
        }
    }
    for model in FALLBACK_MODELS {
        if !chain.iter().any(|existing| existing == model) {
            chain.push((*model).to_string());
        }
    }
    chain.truncate(2);
    chain
}

fn is_retryable(error: &str) -> bool {
    !error.to_lowercase().contains("iptal")
}

/// Bir model denemesine ve tüm denemelere verilen süre bütçesi.
///
/// Ücretsiz modellerin gecikmesi büyük ölçüde değişkendir (5 sn – 140 sn);
/// ilk model bütçeyi aşarsa oturum iptal edilip hızlı yedekle devam edilir.
pub struct PromptBudget {
    /// İlk modelin en fazla sürdüğü süre.
    pub first_attempt: std::time::Duration,
    /// Tüm denemelerin toplam üst sınırı.
    pub total: std::time::Duration,
}

impl PromptBudget {
    /// Özet: hızlı modeller 5-20 sn'de bitiriyor.
    pub const SUMMARY: Self = Self {
        first_attempt: std::time::Duration::from_secs(40),
        total: std::time::Duration::from_secs(75),
    };
    /// Dilekçe: uzun çıktı üretiyor (85-140 sn ölçüm).
    pub const PETITION: Self = Self {
        first_attempt: std::time::Duration::from_secs(90),
        total: std::time::Duration::from_secs(180),
    };
    /// Sohbet: özetten uzun, dilekçeden kısa yanıtlar.
    pub const CHAT: Self = Self {
        first_attempt: std::time::Duration::from_secs(60),
        total: std::time::Duration::from_secs(150),
    };
}

/// Akış destekli metin üretimi: oturum açıldığında `ai://session`, üretim
/// sürerken `ai://progress` olayları yayınlanır; model hata verirse veya
/// bütçeyi aşarsa yedekle otomatik tekrar denenir (`ai://retry`).
pub async fn run_prompt_streaming(
    state: &AppState,
    prompt: &str,
    request_id: Option<&str>,
    budget: PromptBudget,
) -> Result<ChatReply, String> {
    run_prompt_streaming_with_session(state, prompt, request_id, budget, None)
        .await
        .map(|(reply, _session)| reply)
}

/// Oturum korumalı akış (sohbet için).
///
/// `existing_session` verilirse ilk deneme bu oturumda çalışır ve konuşma
/// geçmişi opencode tarafında sürer; yedek modeller yeni oturum açar. Kullanılan
/// oturumun kimliği döner — çağıran bunu veritabanında saklayıp sonraki
/// mesajlarda yeniden kullanır.
pub async fn run_prompt_streaming_with_session(
    state: &AppState,
    prompt: &str,
    request_id: Option<&str>,
    budget: PromptBudget,
    existing_session: Option<&str>,
) -> Result<(ChatReply, Option<String>), String> {
    // Sunucuya erişim yoksa uygulama onu gizli alt süreç olarak başlatır.
    crate::core::opencode_server::ensure_server().await?;

    let chain = model_chain(state).await;
    let mut last_error = "Model seçilemedi.".to_string();
    let started = std::time::Instant::now();
    let current_session: std::sync::Arc<std::sync::Mutex<Option<String>>> = Default::default();

    for (index, model) in chain.iter().enumerate() {
        let remaining = budget
            .total
            .checked_sub(started.elapsed())
            .unwrap_or_default();
        if remaining.is_zero() {
            break;
        }
        let attempt_cap = budget.first_attempt.min(remaining);

        // Yalnızca ilk deneme var olan oturumu kullanır; yedekler yeni açar.
        let attempt_session = if index == 0 { existing_session } else { None };

        let run = run_prompt_once(
            state,
            prompt,
            request_id,
            model,
            &current_session,
            attempt_session,
        );
        match tokio::time::timeout(attempt_cap, run).await {
            Ok(Ok(reply)) => {
                let used_session = current_session
                    .lock()
                    .ok()
                    .and_then(|slot| slot.clone());
                return Ok((reply, used_session));
            }
            Ok(Err(error)) => {
                tracing::warn!("İstek model '{}' ile başarısız: {}", model, error);
                last_error = error.clone();
                if !is_retryable(&error) || index + 1 >= chain.len() {
                    break;
                }
            }
            Err(_elapsed) => {
                tracing::warn!(
                    "Model '{}' {} saniyede bitiremedi; yedeğe geçiliyor",
                    model,
                    attempt_cap.as_secs()
                );
                last_error = format!(
                    "Model {} {} saniyede yanıt vermedi.",
                    model,
                    attempt_cap.as_secs()
                );
                let session_id = current_session
                    .lock()
                    .ok()
                    .and_then(|slot| slot.clone());
                if let Some(session_id) = session_id {
                    client().abort(&session_id).await;
                }
                if index + 1 >= chain.len() {
                    break;
                }
            }
        }

        let next = &chain[index + 1];
        emit_ai_event(
            state,
            "ai://retry",
            serde_json::json!({
                "requestId": request_id,
                "failedModel": model,
                "nextModel": next,
                "error": last_error,
            }),
        );
    }

    Err(last_error)
}

async fn run_prompt_once(
    state: &AppState,
    prompt: &str,
    request_id: Option<&str>,
    model: &str,
    session_slot: &std::sync::Mutex<Option<String>>,
    existing_session: Option<&str>,
) -> Result<ChatReply, String> {
    let client = client();
    let (session_id, reused) = match existing_session {
        Some(id) => (id.to_string(), true),
        None => (client.create_session(Some(model)).await?, false),
    };
    if reused {
        tracing::debug!("Mevcut opencode oturumu kullanılıyor: {}", session_id);
    }
    CANCELLED_SESSIONS.lock().unwrap().remove(&session_id);
    if let Ok(mut slot) = session_slot.lock() {
        *slot = Some(session_id.clone());
    }
    emit_ai_event(
        state,
        "ai://session",
        serde_json::json!({ "requestId": request_id, "sessionId": session_id }),
    );

    let app = state.app_handle.lock().ok().and_then(|guard| guard.clone());
    let progress_request = request_id.map(str::to_string);
    let progress_model = model.to_string();
    let on_progress = move |_session: &str, text: &str| {
        if let Some(app) = app.as_ref() {
            let _ = app.emit(
                "ai://progress",
                serde_json::json!({
                    "requestId": progress_request,
                    "sessionId": _session,
                    "model": progress_model,
                    "text": text,
                }),
            );
        }
    };
    let cancelled_id = session_id.clone();
    let cancelled = move || {
        CANCELLED_SESSIONS
            .lock()
            .map(|set| set.contains(&cancelled_id))
            .unwrap_or(false)
    };

    let hooks = crate::core::opencode_client::PromptHooks {
        on_progress: Some(&on_progress),
        is_cancelled: Some(&cancelled),
    };
    let result = client.prompt_on_session(&session_id, prompt, &hooks).await;
    CANCELLED_SESSIONS.lock().unwrap().remove(&session_id);
    result
}

/// Oturum iptal kaydına bakar (bulut akışı da istek kimliğiyle kullanır).
pub fn is_cancelled(id: &str) -> bool {
    CANCELLED_SESSIONS
        .lock()
        .map(|set| set.contains(id))
        .unwrap_or(false)
}

/// İptal kaydını temizler (istek tamamlandığında/başarısız olduğunda).
pub fn clear_cancelled(id: &str) {
    CANCELLED_SESSIONS.lock().unwrap().remove(id);
}

pub(crate) fn emit_ai_event(state: &AppState, event: &str, payload: serde_json::Value) {
    if let Ok(guard) = state.app_handle.lock() {
        if let Some(app) = guard.as_ref() {
            let _ = app.emit(event, payload);
        }
    }
}

/// Kullanıcının iptal ettiği YZ isteğini hem yoklamada hem opencode'da durdurur.
#[tauri::command]
pub async fn abort_ai_request(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    CANCELLED_SESSIONS.lock().unwrap().insert(session_id.clone());
    emit_ai_event(
        &state,
        "ai://cancelled",
        serde_json::json!({ "sessionId": session_id }),
    );
    client().abort(&session_id).await;
    Ok(())
}
