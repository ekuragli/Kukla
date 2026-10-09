//! Web arama komutları ve sohbet entegrasyonu (Tavily).
//!
//! Gizlilik: arama yalnızca sohbet oturumunda paylaşıma açıkça izin verildiğinde
//! çalışır; API anahtarı şifreli veritabanında tutulur ve frontend'e dönmez.

use tauri::State;

use crate::core::websearch::{SearchDepth, TavilyProvider};
use crate::models::{AiSecretsStatus, AuditEventType};
use crate::AppState;

/// Veritabanındaki web arama anahtarını okur.
pub fn web_search_key(state: &AppState) -> Option<String> {
    state
        .db
        .read()
        .ok()
        .and_then(|db| db.get_ai_secrets().ok())
        .and_then(|secrets| secrets.web_search_api_key)
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

#[tauri::command]
pub async fn get_web_search_status(state: State<'_, AppState>) -> Result<AiSecretsStatus, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    let secrets = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_ai_secrets()
        .map_err(|e| format!("Anahtar durumu okunamadı: {}", e))?;
    Ok(AiSecretsStatus::from_secrets(&secrets))
}

/// Web arama API anahtarını kaydeder; boş/`None` anahtarı siler.
#[tauri::command]
pub async fn set_web_search_key(
    state: State<'_, AppState>,
    key: Option<String>,
) -> Result<AiSecretsStatus, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .set_web_search_api_key(key.as_deref())
        .map_err(|e| format!("Anahtar kaydedilemedi: {}", e))?;

    state
        .audit
        .log(AuditEventType::SettingsChange, Some("web_search_key"));

    let secrets = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_ai_secrets()
        .map_err(|e| format!("Anahtar durumu okunamadı: {}", e))?;
    Ok(AiSecretsStatus::from_secrets(&secrets))
}

/// Doğrudan web araması (sohbet dışından kullanım ve testler için).
#[tauri::command]
pub async fn web_search(    state: State<'_, AppState>,
    query: String,
    max_results: Option<u32>,
    depth: Option<String>,
) -> Result<Vec<crate::core::websearch::WebSearchResult>, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let sanitized = crate::utils::sanitize_query(&query);
    if sanitized.is_empty() {
        return Err("Arama sorgusu boş olamaz.".to_string());
    }
    state
        .audit
        .log(AuditEventType::Search, Some(&format!("web:{}", sanitized)));

    let key = web_search_key(&state)
        .ok_or_else(|| "Web arama anahtarı tanımlı değil. Ayarlar → YZ Altyapısı bölümünden Tavily anahtarını girin.".to_string())?;
    let provider = TavilyProvider::new(key);
    provider
        .search(
            &sanitized,
            max_results.unwrap_or(3),
            SearchDepth::from_optional(depth.as_deref()),
        )
        .await
}

/// Sohbet yanıtı için web kaynakları çeker.
///
/// Hata sessizce yutulmaz: çağıran `notice` olarak kullanıcıya gösterir.
pub async fn chat_web_sources(
    state: &AppState,
    query: &str,
) -> (Vec<crate::core::rag::retrieve::RetrievedSource>, Option<String>) {
    let key = web_search_key(state);
    chat_web_sources_with_key(key.as_deref(), query).await
}

/// Anahtarı verilen web araması (test edilebilirlik için AppState'ten bağımsız).
pub async fn chat_web_sources_with_key(
    api_key: Option<&str>,
    query: &str,
) -> (Vec<crate::core::rag::retrieve::RetrievedSource>, Option<String>) {
    let Some(key) = api_key.map(str::trim).filter(|value| !value.is_empty()) else {
        return (
            Vec::new(),
            Some(
                "İnternet araması etkin ama Tavily anahtarı tanımlı değil; yalnızca yerel arşiv kullanıldı."
                    .to_string(),
            ),
        );
    };

    let provider = TavilyProvider::new(key);
    match provider
        .search(query, CHAT_WEB_MAX_RESULTS, SearchDepth::Basic)
        .await
    {
        Ok(results) => {
            let sources = results
                .into_iter()
                .map(|result| crate::core::rag::retrieve::RetrievedSource {
                    id: format!("web:{}", result.url),
                    label: format!("{} ({})", result.title, result.url),
                    url: Some(result.url),
                    kind: "web",
                    score: result.score,
                    text: crate::utils::truncate_text(&result.content, CHAT_WEB_CHAR_CAP),
                })
                .collect();
            (sources, None)
        }
        Err(error) => (
            Vec::new(),
            Some(format!(
                "İnternet araması yapılamadı: {}. Yanıt yalnızca yerel arşive dayanıyor.",
                error
            )),
        ),
    }
}

/// Sohbette çekilecek web sonucu sayısı (Tavily kredisi korunur).
pub const CHAT_WEB_MAX_RESULTS: u32 = 3;
/// Her web sonucunun prompt'a girecek karakter sınırı.
pub const CHAT_WEB_CHAR_CAP: usize = 800;
