//! Bulut YZ sağlayıcıları — OpenAI uyumlu chat completions istemcisi.
//!
//! Ücretsiz kullanım için hazır preset'ler:
//! - OpenRouter: `:free` sonekli modeller (dakikada 20, günde 50 istek)
//! - Groq: dakikada 30 istek; günlük token tavanı modele göre değişir
//! - Google Gemini: AI Studio (OpenAI uyumlu uç nokta)
//! - Özel: kullanıcının kendi OpenAI uyumlu uç noktası
//!
//! İstekler SSE (`stream: true`) ile akar; kısmi metin `on_delta` ile iletilir.
//! KVKK: bu modda soru metni sağlayıcıya gönderilir — yalnızca sohbet
//! oturumunda paylaşıma izin verildiğinde kullanılır.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Sağlayıcı hazır ayarı.
pub struct ProviderPreset {
    pub id: &'static str,
    pub label: &'static str,
    /// OpenAI uyumlu temel adres.
    pub base_url: &'static str,
    pub models_url: &'static str,
    /// Kota sorgulama uç noktası (yalnızca OpenRouter var).
    pub quota_url: Option<&'static str>,
    pub hint: &'static str,
}

pub const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        id: "openrouter",
        label: "OpenRouter (ücretsiz modeller)",
        base_url: "https://openrouter.ai/api/v1",
        models_url: "https://openrouter.ai/api/v1/models",
        quota_url: Some("https://openrouter.ai/api/v1/key"),
        hint: ":free ile biten modeller ücretsizdir (dakikada 20, günde 50 istek)",
    },
    ProviderPreset {
        id: "groq",
        label: "Groq (ücretsiz katman)",
        base_url: "https://api.groq.com/openai/v1",
        models_url: "https://api.groq.com/openai/v1/models",
        quota_url: None,
        hint: "Dakikada 30 istek; günlük token tavanı modele göre değişir",
    },
    ProviderPreset {
        id: "gemini",
        label: "Google Gemini (AI Studio)",
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        models_url: "https://generativelanguage.googleapis.com/v1beta/openai/models",
        quota_url: None,
        hint: "Ücretsiz katman proje bazlı kotayla çalışır",
    },
    ProviderPreset {
        id: "custom",
        label: "Özel (OpenAI uyumlu)",
        base_url: "",
        models_url: "",
        quota_url: None,
        hint: "Kendi OpenAI uyumlu uç noktanızın adresini girin",
    },
];

pub fn preset(id: &str) -> Option<&'static ProviderPreset> {
    PRESETS.iter().find(|preset| preset.id == id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionMessage {
    pub role: String,
    pub content: String,
}

impl ChatCompletionMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CloudReply {
    pub text: String,
    pub model: Option<String>,
}

pub struct CloudClient {
    base_url: String,
    api_key: String,
    model: String,
    http: reqwest::Client,
}

impl CloudClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(150))
            .build()
            .unwrap_or_default();
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model: model.into(),
            http,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Chat completions akışı: kısmi metinler `on_delta` ile iletilir.
    pub async fn chat_streaming(
        &self,
        messages: &[ChatCompletionMessage],
        on_delta: &mut (dyn FnMut(&str) + Send),
        is_cancelled: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<CloudReply, String> {
        let url = format!("{}/chat/completions", self.base_url);
        let payload = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
        });

        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Sağlayıcıya ulaşılamadı: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(cloud_error_message(status.as_u16(), &body));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut full_text = String::new();

        while let Some(chunk) = stream.next().await {
            if is_cancelled() {
                return Err("İstek iptal edildi.".to_string());
            }
            let chunk = chunk.map_err(|e| format!("Akış okunamadı: {}", e))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // SSE olayları boş satırla ayrılır.
            while let Some(boundary) = buffer.find("\n\n") {
                let event: String = buffer[..boundary].to_string();
                buffer.drain(..boundary + 2);
                for line in event.lines() {
                    let Some(data) = line.strip_prefix("data:") else {
                        continue;
                    };
                    let data = data.trim();
                    if data.is_empty() || data == "[DONE]" {
                        continue;
                    }
                    if let Some(delta) = parse_stream_delta(data) {
                        full_text.push_str(&delta);
                        on_delta(&full_text);
                    }
                }
            }
        }

        if full_text.trim().is_empty() {
            return Err("Sağlayıcı boş yanıt döndürdü.".to_string());
        }

        Ok(CloudReply {
            text: full_text,
            model: Some(self.model.clone()),
        })
    }

    /// Sağlayıcının model listesi (anahtar gerekli).
    pub async fn list_models(&self, models_url: &str) -> Result<Vec<String>, String> {
        if models_url.is_empty() {
            return Ok(Vec::new());
        }
        let response = self
            .http
            .get(models_url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| format!("Model listesi alınamadı: {}", e))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| format!("Model listesi okunamadı: {}", e))?;
        if !status.is_success() {
            return Err(cloud_error_message(status.as_u16(), &body));
        }

        parse_model_list(&body)
    }

    /// Kalan günlük/ücretsiz kota (yalnızca destekleyen sağlayıcılar).
    pub async fn quota(&self, quota_url: &str) -> Result<Option<String>, String> {
        if quota_url.is_empty() {
            return Ok(None);
        }
        let response = self
            .http
            .get(quota_url)
            .bearer_auth(&self.api_key)
            .send()
            .await;

        let Ok(response) = response else {
            return Ok(None);
        };
        if !response.status().is_success() {
            return Ok(None);
        }
        let Ok(body) = response.text().await else {
            return Ok(None);
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) else {
            return Ok(None);
        };

        let free = value.pointer("/data/free_model_daily_requests");
        match (free.and_then(|f| f.get("remaining")), free.and_then(|f| f.get("limit"))) {
            (Some(remaining), Some(limit)) => Ok(Some(format!(
                "{} / {} günlük ücretsiz istek",
                remaining.as_i64().unwrap_or(0),
                limit.as_i64().unwrap_or(0)
            ))),
            _ => Ok(None),
        }
    }
}

/// Tek bir SSE veri satırından metin parçasını çıkarır.
pub fn parse_stream_delta(data: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    let delta = value.pointer("/choices/0/delta")?;
    let text = delta.get("content")?.as_str()?;
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[derive(Deserialize)]
struct ModelListResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

/// Model listesi yanıtını adlara çevirir.
pub fn parse_model_list(body: &str) -> Result<Vec<String>, String> {
    let parsed: ModelListResponse =
        serde_json::from_str(body).map_err(|e| format!("Model listesi çözümlenemedi: {}", e))?;
    let mut ids: Vec<String> = parsed.data.into_iter().map(|entry| entry.id).collect();
    ids.sort();
    Ok(ids)
}

/// HTTP durumunu kullanıcı-dostu Türkçe hataya çevirir.
pub fn cloud_error_message(status: u16, body: &str) -> String {
    let detail = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("boş yanıt");
    let detail: String = detail.chars().take(200).collect();
    match status {
        401 | 403 => "API anahtarı geçersiz. Ayarlar → YZ Altyapısı bölümünden kontrol edin."
            .to_string(),
        402 => "Sağlayıcı bakiyesi/kredisi yetersiz.".to_string(),
        429 => "İstek sınırı aşıldı (dakikalık veya günlük kota). Bir süre bekleyip tekrar deneyin."
            .to_string(),
        _ => format!("Sağlayıcı hatası (HTTP {}): {}", status, detail),
    }
}
