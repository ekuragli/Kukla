//! `opencode serve` HTTP API istemcisi.
//!
//! Sunucu terminalde manuel olarak başlatılır:
//! `opencode serve --port 4096`
//!
//! Kullanılan uçlar (opencode 1.18 `/doc` OpenAPI):
//! `GET  /api/health`                          -> `{ healthy: true }`
//! `POST /api/session`                         -> `{ data: { id: "ses_..." } }`
//! `POST /api/session/{id}/prompt`             -> `{ prompt: { text } }`
//! `GET  /api/session/{id}/message?order=asc`  -> `{ data: [ msg... ] }`

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:4096";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const REPLY_TIMEOUT: Duration = Duration::from_secs(240);
const POLL_INTERVAL: Duration = Duration::from_millis(400);

/// Terminalde çalışan opencode sunucusunun durumu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpencodeStatus {
    pub healthy: bool,
    pub base_url: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    /// Sunucuyu Kukla mı başlattı (gizli alt süreç)?
    #[serde(default)]
    pub managed_by_app: bool,
}

/// Tek bir istemin (prompt) tamamlanmış sonucu.
#[derive(Debug, Clone, Default)]
pub struct ChatReply {
    pub text: String,
    pub model: Option<String>,
    pub agent: Option<String>,
}

/// opencode sunucusunun bildirdiği bir model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpencodeModel {
    pub id: String,
    pub provider_id: String,
    #[serde(default)]
    pub enabled: bool,
}

impl OpencodeModel {
    /// `providerID/modelID` biçiminde okunabilir etiket.
    pub fn label(&self) -> String {
        format!("{}/{}", self.provider_id, self.id)
    }
}

/// `"providerID/modelID"` (veya sadece `modelID`) biçimindeki model seçimini ayırır.
pub fn split_model(selection: &str) -> (String, String) {
    match selection.trim().rsplit_once('/') {
        Some((provider, id)) if !provider.is_empty() && !id.is_empty() => {
            (provider.to_string(), id.to_string())
        }
        _ => ("opencode".to_string(), selection.trim().to_string()),
    }
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Deserialize)]
struct SessionInfo {
    id: String,
}

#[derive(Deserialize)]
struct ModelListPage {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(rename = "providerID", default)]
    provider_id: String,
    #[serde(default)]
    enabled: Option<bool>,
}

#[derive(Deserialize)]
struct MessagesPage {
    #[serde(default)]
    data: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct SessionListPage {
    #[serde(default)]
    data: Vec<SessionInfo>,
}

#[derive(Deserialize)]
struct PermissionList {
    #[serde(default)]
    data: Vec<serde_json::Value>,
}

pub struct OpencodeClient {
    base_url: String,
    http: reqwest::Client,
}

impl OpencodeClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let base = base_url.into();
        let base = if base.is_empty() {
            DEFAULT_BASE_URL.to_string()
        } else {
            base.trim_end_matches('/').to_string()
        };
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self { base_url: base, http }
    }

    pub fn from_setting(url: &str) -> Self {
        Self::new(url)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value, String> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| http_error(&self.base_url, &e))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("opencode yanıtı okunamadı: {}", e))?;
        if !status.is_success() {
            return Err(format!(
                "opencode HTTP {}: {}",
                status.as_u16(),
                first_line(&body)
            ));
        }
        serde_json::from_str(&body)
            .map_err(|e| format!("opencode yanıtı çözümlenemedi: {}", e))
    }

    async fn post_json(
        &self,
        url: &str,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let resp = self
            .http
            .post(url)
            .json(payload)
            .send()
            .await
            .map_err(|e| http_error(&self.base_url, &e))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("opencode yanıtı okunamadı: {}", e))?;
        if !status.is_success() {
            return Err(format!(
                "opencode HTTP {}: {}",
                status.as_u16(),
                first_line(&body)
            ));
        }
        if body.trim().is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&body)
            .map_err(|e| format!("opencode yanıtı çözümlenemedi: {}", e))
    }

    /// Sunucu ayakta mı?
    pub async fn health(&self) -> Result<bool, String> {
        let url = format!("{}/api/health", self.base_url);
        let value = self.get_json(&url).await?;
        Ok(value
            .get("healthy")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Yeni oturum açar. `model`: `providerID/modelID`; `None` opencode varsayılanı.
    pub async fn create_session(&self, model: Option<&str>) -> Result<String, String> {
        let url = format!("{}/api/session", self.base_url);
        let payload = match model.filter(|value| !value.trim().is_empty()) {
            Some(selection) => {
                let (provider, id) = split_model(selection);
                serde_json::json!({ "model": { "providerID": provider, "id": id } })
            }
            None => serde_json::json!({}),
        };
        let value = self.post_json(&url, &payload).await?;
        let id = serde_json::from_value::<Envelope<SessionInfo>>(value)
            .map_err(|e| format!("opencode oturum yanıtı çözümlenemedi: {}", e))?
            .data
            .id;
        Ok(id)
    }

    /// Oturumun opencode tarafından kabul edilen modeli (`providerID/modelID`).
    pub async fn session_model(&self, session_id: &str) -> Option<String> {
        let url = format!("{}/api/session/{}", self.base_url, session_id);
        let value = self.get_json(&url).await.ok()?;
        let model = value.pointer("/data/model").or_else(|| value.pointer("/model"))?;
        let id = model.get("id")?.as_str()?.to_string();
        let provider = model
            .get("providerID")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("opencode");
        Some(format!("{}/{}", provider, id))
    }

    async fn send_prompt(&self, session_id: &str, text: &str) -> Result<(), String> {
        let url = format!("{}/api/session/{}/prompt", self.base_url, session_id);
        let payload = serde_json::json!({ "prompt": { "text": text } });
        self.post_json(&url, &payload).await?;
        Ok(())
    }

    async fn messages(&self, session_id: &str) -> Result<Vec<serde_json::Value>, String> {
        let url = format!(
            "{}/api/session/{}/message?order=asc&limit=50",
            self.base_url, session_id
        );
        let value = self.get_json(&url).await?;
        let page: MessagesPage = serde_json::from_value(value)
            .map_err(|e| format!("opencode mesaj yanıtı çözümlenemedi: {}", e))?;
        Ok(page.data)
    }

    /// Yetkilendirme bekleyen araç isteklerini reddeder (metin üretimi dışında
    /// hiçbir araca izin verilmez).
    async fn reject_pending_permissions(&self, session_id: &str) {
        let url = format!(
            "{}/api/session/{}/permission",
            self.base_url, session_id
        );
        let Ok(value) = self.get_json(&url).await else {
            return;
        };
        let Ok(page) = serde_json::from_value::<PermissionList>(value) else {
            return;
        };
        for request in page.data {
            let Some(id) = request.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let reply_url = format!(
                "{}/api/session/{}/permission/{}/reply",
                self.base_url, session_id, id
            );
            let payload = serde_json::json!({ "reply": "reject" });
            let _ = self.post_json(&reply_url, &payload).await;
        }
    }

    /// opencode sunucusunun sunduğu modeller (etkin olanlar).
    pub async fn list_models(&self) -> Vec<OpencodeModel> {
        let url = format!("{}/api/model", self.base_url);
        let Ok(value) = self.get_json(&url).await else {
            return Vec::new();
        };
        let Ok(page) = serde_json::from_value::<ModelListPage>(value) else {
            return Vec::new();
        };
        page.data
            .into_iter()
            .filter(|entry| entry.enabled.unwrap_or(true))
            .map(|entry| OpencodeModel {
                id: entry.id,
                provider_id: if entry.provider_id.is_empty() {
                    "opencode".to_string()
                } else {
                    entry.provider_id
                },
                enabled: true,
            })
            .collect()
    }

    /// Yeni oturumda tek seferlik istek çalıştırır ve modelin metin yanıtını döndürür.
    ///
    /// `model`: `"providerID/modelID"`; `None` opencode'un kendi varsayılanı kullanılır.
    pub async fn chat(&self, prompt: &str, model: Option<&str>) -> Result<ChatReply, String> {
        self.chat_with_hooks(prompt, model, &PromptHooks::NONE).await
    }

    /// İlerleme/iptal geri çağrılarıyla çalışan `chat`.
    pub async fn chat_with_hooks(
        &self,
        prompt: &str,
        model: Option<&str>,
        hooks: &PromptHooks<'_>,
    ) -> Result<ChatReply, String> {
        let session_id = self.create_session(model).await?;
        self.prompt_on_session(&session_id, prompt, hooks).await
    }

    /// Var olan bir oturumda istek çalıştırır. Üretim sürerken her yoklamada
    /// `on_progress` ile kısmi metin, iptal durumunda `is_cancelled` dinlenir.
    pub async fn prompt_on_session(
        &self,
        session_id: &str,
        prompt: &str,
        hooks: &PromptHooks<'_>,
    ) -> Result<ChatReply, String> {
        self.send_prompt(session_id, prompt).await?;
        self.wait_for_reply_with(session_id, hooks).await
    }

    /// Kullanıcı iptalinde opencode tarafında üretimi durdurur.
    pub async fn abort(&self, session_id: &str) {
        let url = format!("{}/api/session/{}/abort", self.base_url, session_id);
        let _ = self.post_json(&url, &serde_json::json!({})).await;
    }

    async fn wait_for_reply_with(
        &self,
        session_id: &str,
        hooks: &PromptHooks<'_>,
    ) -> Result<ChatReply, String> {
        let deadline = Instant::now() + REPLY_TIMEOUT;
        let mut last_emitted = String::new();
        loop {
            if let Some(is_cancelled) = hooks.is_cancelled {
                if is_cancelled() {
                    return Err("İstek iptal edildi.".to_string());
                }
            }

            tokio::time::sleep(POLL_INTERVAL).await;

            self.reject_pending_permissions(session_id).await;

            let messages = self.messages(session_id).await?;
            if let Some(on_progress) = hooks.on_progress {
                if let Some(partial) = partial_assistant_text(&messages) {
                    if !partial.is_empty() && partial != last_emitted {
                        last_emitted.clone_from(&partial);
                        on_progress(session_id, &partial);
                    }
                }
            }
            if let Some(reply) = latest_completed_assistant(&messages) {
                return reply;
            }

            if Instant::now() >= deadline {
                return Err(format!(
                    "opencode{} yanıt vermedi ({} saniye). Sunucunun açık olduğunu ve \
                     modelin çalıştığını kontrol edin: opencode serve --port {}",
                    self.base_url,
                    REPLY_TIMEOUT.as_secs(),
                    port_of(&self.base_url)
                ));
            }
        }
    }

    /// Sunucu durumu + son kullanılan model (TUI'de seçilen model).
    pub async fn status(&self) -> OpencodeStatus {
        match self.health().await {
            Ok(false) => OpencodeStatus {
                healthy: false,
                base_url: self.base_url.clone(),
                detail: Some("opencode sunucusu yanıt vermedi.".to_string()),
                model: None,
                agent: None,
                managed_by_app: false,
            },
            Err(e) => OpencodeStatus {
                healthy: false,
                base_url: self.base_url.clone(),
                detail: Some(e),
                model: None,
                agent: None,
                managed_by_app: false,
            },
            Ok(true) => {
                let (model, agent) = self.last_model().await;
                OpencodeStatus {
                    healthy: true,
                    base_url: self.base_url.clone(),
                    detail: None,
                    model,
                    agent,
                    managed_by_app: false,
                }
            }
        }
    }

    /// Son oturumdaki son tamamlanmış model yanıtından model bilgisini okur.
    async fn last_model(&self) -> (Option<String>, Option<String>) {
        let url = format!("{}/api/session?limit=1", self.base_url);
        let Ok(value) = self.get_json(&url).await else {
            return (None, None);
        };
        let Ok(page) = serde_json::from_value::<SessionListPage>(value) else {
            return (None, None);
        };
        let Some(session) = page.data.into_iter().next() else {
            return (None, None);
        };
        let Ok(messages) = self.messages(&session.id).await else {
            return (None, None);
        };
        let mut best: Option<(i64, Option<String>, Option<String>)> = None;
        for message in &messages {
            if message.get("type").and_then(|v| v.as_str()) != Some("assistant") {
                continue;
            }
            let created = message
                .pointer("/time/created")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if best.as_ref().is_some_and(|(last, _, _)| created <= *last) {
                continue;
            }
            let model = message
                .pointer("/model/providerID")
                .and_then(|v| v.as_str())
                .zip(message.pointer("/model/id").and_then(|v| v.as_str()))
                .map(|(provider, id)| format!("{}/{}", provider, id))
                .or_else(|| {
                    message
                        .pointer("/model/id")
                        .and_then(|v| v.as_str())
                        .map(|id| id.to_string())
                });
            let agent = message
                .get("agent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            best = Some((created, model, agent));
        }
        match best {
            Some((_, model, agent)) => (model, agent),
            None => (None, None),
        }
    }
}

/// YOKLAMA sırasındaki ilerleme/iptal geri çağrıları.
///
/// `on_progress(session_id, partial_text)` model yanıtının (kısmi) metnini her
/// değiştiğinde çağrılır; `is_cancelled` gerçek olduğunda yoklama döngüsü hemen
/// `"İstek iptal edildi."` hatasıyla durur.
pub struct PromptHooks<'a> {
    pub on_progress: Option<&'a (dyn Fn(&str, &str) + Send + Sync)>,
    pub is_cancelled: Option<&'a (dyn Fn() -> bool + Send + Sync)>,
}

impl PromptHooks<'_> {
    pub const NONE: Self = Self {
        on_progress: None,
        is_cancelled: None,
    };
}

/// Üretim sürerkenki (henüz tamamlanmamış) asistan metnini seçer.
fn partial_assistant_text(messages: &[serde_json::Value]) -> Option<String> {
    let mut chosen: Option<&serde_json::Value> = None;
    for message in messages {
        if message.get("type").and_then(|v| v.as_str()) == Some("assistant") {
            chosen = Some(message);
        }
    }
    let text = extract_text(chosen?);
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Tamamlanmış son asistan mesajını seçer; hata varsa Err döner.
fn latest_completed_assistant(messages: &[serde_json::Value]) -> Option<Result<ChatReply, String>> {
    let mut chosen: Option<&serde_json::Value> = None;
    for message in messages {
        if message.get("type").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }
        if !is_completed(message) {
            continue;
        }
        chosen = Some(message);
    }

    let message = chosen?;
    let model = message
        .pointer("/model/providerID")
        .and_then(|v| v.as_str())
        .zip(message.pointer("/model/id").and_then(|v| v.as_str()))
        .map(|(provider, id)| format!("{}/{}", provider, id))
        .or_else(|| {
            message
                .pointer("/model/id")
                .and_then(|v| v.as_str())
                .map(|id| id.to_string())
        });
    let agent = message
        .get("agent")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if let Some(error) = message.get("error").filter(|e| !e.is_null()) {
        let detail = error
            .get("message")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| error.to_string());
        return Some(Err(format!("opencode model hatası: {}", detail)));
    }

    let text = extract_text(message);
    if text.trim().is_empty() {
        return Some(Err(
            "Model boş yanıt döndürdü. Terminalse model seçiminizi kontrol edin.".to_string(),
        ));
    }

    Some(Ok(ChatReply { text, model, agent }))
}

fn extract_text(message: &serde_json::Value) -> String {
    let mut out = String::new();
    if let Some(parts) = message.get("content").and_then(|c| c.as_array()) {
        for part in parts {
            if part.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    out.push_str(text);
                }
            }
        }
    }
    if out.is_empty() {
        if let Some(text) = message.get("text").and_then(|t| t.as_str()) {
            out.push_str(text);
        }
    }
    out
}

fn is_completed(message: &serde_json::Value) -> bool {
    if message.get("error").is_some_and(|e| !e.is_null()) {
        return true;
    }
    message.pointer("/time/completed").is_some()
        || message.get("finish").is_some_and(|f| !f.is_null())
}

fn http_error(base_url: &str, err: &reqwest::Error) -> String {
    if err.is_connect() || err.is_timeout() {
        return format!(
            "opencode sunucusuna ulaşılamadı ({}). Terminalde başlatın: opencode serve --port {}",
            base_url,
            port_of(base_url)
        );
    }
    format!("opencode bağlantı hatası: {}", err)
}

fn port_of(base_url: &str) -> String {
    base_url
        .rsplit_once(':')
        .and_then(|(_, port)| {
            let digits: String = port.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                None
            } else {
                Some(digits)
            }
        })
        .unwrap_or_else(|| "4096".to_string())
}

fn first_line(body: &str) -> String {
    body.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("boş yanıt")
        .chars()
        .take(200)
        .collect()
}
