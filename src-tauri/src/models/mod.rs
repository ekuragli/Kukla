#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub id: i64,
    pub query_text: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub pdf_path: Option<String>,
    pub pdf_hash: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewCase {
    pub query_text: String,
    pub pdf_path: Option<String>,
    pub pdf_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: i64,
    pub case_id: i64,
    pub source: DecisionSource,
    pub esas_no: Option<String>,
    pub karar_no: Option<String>,
    pub karar_tarihi: Option<NaiveDate>,
    pub daire: Option<String>,
    pub summary: Option<String>,
    pub ratio_decidendi: Option<String>,
    pub similarity_score: Option<f64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDecision {
    pub case_id: i64,
    pub source: DecisionSource,
    pub esas_no: Option<String>,
    pub karar_no: Option<String>,
    pub karar_tarihi: Option<NaiveDate>,
    pub daire: Option<String>,
    pub summary: Option<String>,
    pub ratio_decidendi: Option<String>,
    pub similarity_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionSource {
    Yargitay,
    Danistay,
    Bam,
    UserUploaded,
}

impl DecisionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            DecisionSource::Yargitay => "yargitay",
            DecisionSource::Danistay => "danistay",
            DecisionSource::Bam => "bam",
            DecisionSource::UserUploaded => "user_uploaded",
        }
    }

}

impl std::str::FromStr for DecisionSource {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "yargitay" => Ok(DecisionSource::Yargitay),
            "danistay" => Ok(DecisionSource::Danistay),
            "bam" => Ok(DecisionSource::Bam),
            "user_uploaded" => Ok(DecisionSource::UserUploaded),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHistory {
    pub id: i64,
    pub case_id: i64,
    pub query_text: String,
    pub result_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: i64,
    pub event_type: AuditEventType,
    pub event_details: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    AppOpen,
    AppLock,
    LoginSuccess,
    LoginFail,
    Search,
    Summarize,
    PetitionDraft,
    Export,
    Convert,
    PersonalArchiveAdd,
    PersonalArchiveDelete,
    SettingsChange,
    Chat,
}

impl AuditEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditEventType::AppOpen => "app_open",
            AuditEventType::AppLock => "app_lock",
            AuditEventType::LoginSuccess => "login_success",
            AuditEventType::LoginFail => "login_fail",
            AuditEventType::Search => "search",
            AuditEventType::Summarize => "summarize",
            AuditEventType::PetitionDraft => "petition_draft",
            AuditEventType::Export => "export",
            AuditEventType::Convert => "convert",
            AuditEventType::PersonalArchiveAdd => "personal_archive_add",
            AuditEventType::PersonalArchiveDelete => "personal_archive_delete",
            AuditEventType::SettingsChange => "settings_change",
            AuditEventType::Chat => "chat",
        }
    }

}

impl std::str::FromStr for AuditEventType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "app_open" => Ok(AuditEventType::AppOpen),
            "app_lock" => Ok(AuditEventType::AppLock),
            "login_success" => Ok(AuditEventType::LoginSuccess),
            "login_fail" => Ok(AuditEventType::LoginFail),
            "search" => Ok(AuditEventType::Search),
            "summarize" => Ok(AuditEventType::Summarize),
            "petition_draft" => Ok(AuditEventType::PetitionDraft),
            "export" => Ok(AuditEventType::Export),
            "personal_archive_add" => Ok(AuditEventType::PersonalArchiveAdd),
            "personal_archive_delete" => Ok(AuditEventType::PersonalArchiveDelete),
            "settings_change" => Ok(AuditEventType::SettingsChange),
            "chat" => Ok(AuditEventType::Chat),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub decision: Decision,
    pub similarity_percent: u8,
    pub snippet: String,
    #[serde(default)]
    pub online_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub summary: String,
    pub ratio_decidendi: String,
    pub source_citation: String,
    pub hallucination_score: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PetitionType {
    Genel,
    Dava,
    Cevap,
    Itiraz,
    Istinaf,
    Temyiz,
    DelilBildirme,
}

impl PetitionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PetitionType::Genel => "genel",
            PetitionType::Dava => "dava",
            PetitionType::Cevap => "cevap",
            PetitionType::Itiraz => "itiraz",
            PetitionType::Istinaf => "istinaf",
            PetitionType::Temyiz => "temyiz",
            PetitionType::DelilBildirme => "delil_bildirme",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PetitionType::Genel => "Genel Dilekçe",
            PetitionType::Dava => "Dava Dilekçesi",
            PetitionType::Cevap => "Cevap Dilekçesi",
            PetitionType::Itiraz => "İtiraz Dilekçesi",
            PetitionType::Istinaf => "İstinaf Dilekçesi",
            PetitionType::Temyiz => "Temyiz Dilekçesi",
            PetitionType::DelilBildirme => "Delil Bildirme",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PetitionTemplateInfo {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PetitionDraftResponse {
    pub draft: String,
    pub cited_decisions: String,
    pub disclaimer: String,
    pub hallucination_score: f64,
    pub template_type: String,
    pub template_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub language: String,
    pub auto_lock_minutes: u32,
    pub log_retention_days: u32,
    pub personal_archive_enabled: bool,
    /// opencode modeli (`providerID/modelID`). Boş/Yok: opencode'un kendi varsayılanı.
    #[serde(default)]
    pub ai_model: Option<String>,
    /// Sohbetin kullanacağı bulut sağlayıcısı (preset kimliği veya `custom`).
    #[serde(default)]
    pub cloud_provider: Option<String>,
    /// Sağlayıcıdaki model kimliği.
    #[serde(default)]
    pub cloud_model: Option<String>,
    /// `custom` presetinde OpenAI uyumlu temel adres.
    #[serde(default)]
    pub cloud_base_url: Option<String>,
}

/// YZ altyapısının durumu (opencode sunucusu + yerel embedding modeli).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStatus {
    pub opencode: crate::core::opencode_client::OpencodeStatus,
    pub embedding: EmbeddingStatus,
    /// opencode sunucusunun sunduğu modeller (sunucu sağlıklıysa).
    #[serde(default)]
    pub models: Vec<crate::core::opencode_client::OpencodeModel>,
    /// Ayarlar'da seçilen model; `None` varsayılan.
    #[serde(default)]
    pub selected_model: Option<String>,
}

/// Yerel embedding modelinin durumu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingStatus {
    /// `ready` | `loading` | `missing` | `error`
    pub state: String,
    pub dim: usize,
    pub model: String,
    pub path: String,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfParseResult {
    pub text: String,
    pub page_count: u32,
    pub hash: String,
    pub warning: Option<String>,
    #[serde(default)]
    pub suggested_query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkUploadResult {
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFormat {
    pub format: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub id: String,
    pub source: String,
    pub esas_no: Option<String>,
    pub karar_no: Option<String>,
    pub karar_tarihi: Option<NaiveDate>,
    pub daire: Option<String>,
    pub text_chunk: String,
    pub metadata_json: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ExportTarget {
    Docx,
    Pdf,
    Txt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDecisionContext {
    pub decision_id: i64,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub esas_no: Option<String>,
    #[serde(default)]
    pub karar_no: Option<String>,
    #[serde(default)]
    pub daire: Option<String>,
    #[serde(default)]
    pub full_text: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub ratio_decidendi: Option<String>,
    #[serde(default)]
    pub source_citation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSearchOutput {
    pub results: Vec<SearchResult>,
    pub page: u32,
    pub has_more: bool,
    pub search_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: i64,
    pub title: String,
    /// Oturumun açıldığı opencode oturumu (çok turlu süreklilik).
    #[serde(default)]
    pub opencode_session_id: Option<String>,
    /// Oturum açılırken kullanılan model (model değişirse oturum yenilenir).
    #[serde(default)]
    pub model: Option<String>,
    /// KVKK: oturumda dışarıya veri paylaşımına izin verilmiş mi.
    #[serde(default)]
    pub allow_external: bool,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    /// Liste görünümü için mesaj sayısı.
    #[serde(default)]
    pub message_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub session_id: i64,
    /// `user` | `assistant`
    pub role: String,
    pub content: String,
    /// Asistan yanıtının dayandığı kaynaklar (JSON array).
    #[serde(default)]
    pub sources_json: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Bir sohbet yanıtının dayandığı kaynak.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSource {
    /// Görüntülenecek etiket (esas/karar/daire veya dosya adı).
    pub label: String,
    #[serde(default)]
    pub url: Option<String>,
    /// `local` | `web`
    pub kind: String,
    #[serde(default)]
    pub score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSendResponse {
    pub message_id: i64,
    pub reply: String,
    #[serde(default)]
    pub sources: Vec<ChatSource>,
    /// İnternet araması atlandığında/başarısız olduğunda kullanıcıya gösterilen not.
    #[serde(default)]
    pub notice: Option<String>,
}

/// YZ sağlayıcı API anahtarları (yalnızca sunucu tarafı; frontend'e asla
/// serileştirilmez — yerine `AiSecretsStatus` döner).
#[derive(Debug, Clone, Default)]
pub struct AiSecrets {
    pub web_search_api_key: Option<String>,
    pub cloud_api_key: Option<String>,
}

/// Frontend'e dönen anahtar durumu: anahtarın kendisi değil, yalnızca varlığı
/// ve son karakterler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSecretsStatus {
    pub has_web_search_key: bool,
    #[serde(default)]
    pub web_search_key_hint: Option<String>,
    pub has_cloud_key: bool,
    #[serde(default)]
    pub cloud_key_hint: Option<String>,
}

impl AiSecretsStatus {
    pub fn from_secrets(secrets: &AiSecrets) -> Self {
        Self {
            has_web_search_key: secrets.web_search_api_key.is_some(),
            web_search_key_hint: secrets.web_search_api_key.as_deref().map(mask_key),
            has_cloud_key: secrets.cloud_api_key.is_some(),
            cloud_key_hint: secrets.cloud_api_key.as_deref().map(mask_key),
        }
    }
}

/// Bulut sağlayıcı yapılandırmasının arayüze dönen durumu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudStatus {
    /// Ayarlanmış sağlayıcı kimliği (yoksa `None`).
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub has_key: bool,
    pub key_hint: Option<String>,
    /// Sağlayıcının sunduğu modeller (listeleme destekleniyorsa).
    #[serde(default)]
    pub models: Vec<String>,
    /// Kalan kota açıklaması (OpenRouter).
    #[serde(default)]
    pub quota: Option<String>,
    /// Sohbetin şu anki YZ arka ucu: `cloud` | `opencode`.
    pub chat_backend: String,
    /// Mevcut preset'ler (arayüzde seçim listesi için).
    #[serde(default)]
    pub presets: Vec<CloudPresetInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudPresetInfo {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub hint: String,
    /// Özel adres girilmesi gerekiyor mu?
    pub needs_base_url: bool,
    /// Sağlayıcı model listesi sunuyor mu?
    pub supports_models: bool,
}

/// Anahtarın son 4 karakterini gösterir (`••••abc4`).
pub fn mask_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.chars().count() <= 4 {
        return "••••".to_string();
    }
    let suffix: String = trimmed.chars().skip(trimmed.chars().count() - 4).collect();
    format!("••••{}", suffix)
}
