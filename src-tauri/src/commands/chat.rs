//! Sohbet komutları: kalıcı oturumlar, RAG destekli yanıtlar ve opencode
//! oturum sürekliliği.
//!
//! Akış: kullanıcı mesajı SQLite'a yazılır → sorgu gömülüp vektör deposu
//! taranır → geçmiş penceresi + kaynaklarla prompt kurulur → opencode
//! oturumunda üretilir → yanıt ve kaynaklar geri yazılır.

use tauri::State;
use tauri::Emitter;

use crate::core::rag::retrieve::{retrieve_sources, RetrievedSource};
use crate::models::{AuditEventType, ChatMessage, ChatSendResponse, ChatSession, ChatSource};
use crate::AppState;

/// Prompt'a dahil edilen son mesaj sayısı (token bütçesi için sınırlı).
const HISTORY_WINDOW: usize = 8;
/// Geçmiş metninin toplam karakter sınırı.
const HISTORY_CHAR_CAP: usize = 6_000;
/// Vektör deposundan çekilecek kaynak parçası sayısı.
const RAG_TOP_K: u32 = 5;
/// Her kaynak parçasının prompt'a girecek karakter sınırı.
const RAG_CHUNK_CHAR_CAP: usize = 1_500;
/// Tek kullanıcı mesajının karakter sınırı.
const MAX_MESSAGE_CHARS: usize = 4_000;
/// İlk mesajdan türetilen oturum başlığı uzunluğu.
const TITLE_MAX_CHARS: usize = 40;

const CHAT_SYSTEM_PROMPT: &str = "Sen Türk hukuku ve Yargıtay/Danıştay içtihatları konusunda uzman bir hukuk asistanısın.\n\
ZORUNLU KURALLAR:\n\
- Yanıt dili TÜRKÇE olacak.\n\
- Yalnızca KAYNAK METİNLER ile genel hukuki bilgilerine dayanarak yanıt ver.\n\
- Her önemli iddia için dayandığın kaynağı [Kaynak N] biçiminde belirt.\n\
- Kaynaklarda bilgi yoksa bunu açıkça söyle; metin uydurma.\n\
- Dosya, terminal, tarayıcı veya başka bir araç KULLANMA; yalnızca yanıt metnini üret.\n\
- Yanıtın sonuna şu uyarıyı ekle: 'Bu yanıt bilgilendirme amaçlıdır; hukuki danışmanlık yerine geçmez.'";

#[tauri::command]
pub async fn chat_new_session(
    state: State<'_, AppState>,
    title: Option<String>,
    allow_external: Option<bool>,
) -> Result<i64, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    let title = title
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Yeni sohbet".to_string());
    let allow_external = allow_external.unwrap_or(false);

    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .create_chat_session(&title, allow_external)
        .map_err(|e| format!("Sohbet oturumu oluşturulamadı: {}", e))
}

#[tauri::command]
pub async fn chat_sessions(state: State<'_, AppState>) -> Result<Vec<ChatSession>, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .list_chat_sessions()
        .map_err(|e| format!("Sohbet oturumları okunamadı: {}", e))
}

#[tauri::command]
pub async fn chat_history(
    state: State<'_, AppState>,
    session_id: i64,
) -> Result<Vec<ChatMessage>, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .list_chat_messages(session_id)
        .map_err(|e| format!("Sohbet geçmişi okunamadı: {}", e))
}

#[tauri::command]
pub async fn chat_delete_session(state: State<'_, AppState>, session_id: i64) -> Result<(), String> {
    crate::commands::auth_guard::require_auth(&state)?;
    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .delete_chat_session(session_id)
        .map_err(|e| format!("Sohbet oturumu silinemedi: {}", e))?;
    Ok(())
}

/// Dışarıya veri paylaşımı tercihini günceller (KVKK).
#[tauri::command]
pub async fn chat_set_external(
    state: State<'_, AppState>,
    session_id: i64,
    allow_external: bool,
) -> Result<(), String> {
    crate::commands::auth_guard::require_auth(&state)?;
    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .set_chat_session_external(session_id, allow_external)
        .map_err(|e| format!("Paylaşım tercihi güncellenemedi: {}", e))
}

#[tauri::command]
pub async fn chat_send(
    state: State<'_, AppState>,
    session_id: i64,
    message: String,
    request_id: Option<String>,
) -> Result<ChatSendResponse, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let question = message.trim().to_string();
    if question.is_empty() {
        return Err("Mesaj boş olamaz.".to_string());
    }
    let question = crate::utils::truncate_text(&question, MAX_MESSAGE_CHARS);

    let session = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_chat_session(session_id)
        .map_err(|e| format!("Sohbet oturumu okunamadı: {}", e))?
        .ok_or_else(|| "Sohbet oturumu bulunamadı.".to_string())?;

    state
        .audit
        .log(AuditEventType::Chat, Some(&format!("chat_session_{}", session_id)));

    // Kullanıcı mesajını hemen kaydet; ilk mesajsa oturum başlığı ondan türetilir.
    let existing = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .list_chat_messages(session_id)
        .unwrap_or_default();
    let is_first = existing.is_empty();
    drop(existing);

    if let Ok(db) = state.db.read() {
        if let Err(error) = db.append_chat_message(session_id, "user", &question, None) {
            tracing::warn!("Kullanıcı mesajı kaydedilemedi: {}", error);
        }
        if is_first {
            let title = crate::utils::truncate_text(&question, TITLE_MAX_CHARS);
            let _ = db.set_chat_session_title(session_id, &title);
        }
    }

    // RAG: sorguyu göm, vektör deposundan kaynak çek.
    let mut sources = retrieve_sources(&state, &question, RAG_TOP_K, RAG_CHUNK_CHAR_CAP)
        .await
        .unwrap_or_default();

    // İnternet araması yalnızca oturumda paylaşıma izin verildiğinde çalışır (KVKK).
    let mut notice: Option<String> = None;
    if session.allow_external {
        let (web_sources, web_notice) =
            crate::commands::websearch::chat_web_sources(&state, &question).await;
        sources.extend(web_sources);
        notice = web_notice;
    }

    // Prompt'a son mesaj (soru) hariç geçmiş penceresi girer.
    let history = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .list_chat_messages(session_id)
        .unwrap_or_default();
    let prior_end = history.len().saturating_sub(1);
    let parts = build_chat_parts(&history[..prior_end], &sources, &question);
    let prompt = render_opencode_prompt(&parts);

    // Model değiştiyse eski opencode oturumu kullanılamaz.
    let current_model = crate::commands::opencode::selected_model(&state);
    let reuse_session = match (&session.opencode_session_id, &session.model, &current_model) {
        (Some(id), Some(session_model), Some(current)) if session_model == current => {
            Some(id.clone())
        }
        _ => None,
    };

    // Arka uç: yapılandırılmış bulut sağlayıcısı varsa ve oturumda paylaşıma
    // izin verilmişse bulut, aksi halde yerel opencode kullanılır.
    let mut backend_note: Option<String> = None;
    let cloud = match crate::commands::cloud::resolve_cloud_backend(&state) {
        Ok(backend) => backend,
        Err(error) => {
            tracing::debug!("Bulut arka ucu kullanılamıyor: {}", error);
            None
        }
    };

    let (reply_text, used_opencode_session) = match cloud {
        Some(backend) if session.allow_external => {
            let request_key = request_id.clone().unwrap_or_else(|| format!("chat_{}", session_id));
            crate::commands::opencode::emit_ai_event(
                &state,
                "ai://session",
                serde_json::json!({ "requestId": request_id, "sessionId": request_key }),
            );

            // Emit için yalnızca app handle yakalanır (future'ın Send kalması için).
            let app = state.app_handle.lock().ok().and_then(|guard| guard.clone());
            let progress_request = request_id.clone();
            let mut on_delta = move |full: &str| {
                if let Some(app) = app.as_ref() {
                    let _ = app.emit(
                        "ai://progress",
                        serde_json::json!({ "requestId": progress_request, "text": full }),
                    );
                }
            };
            let client = crate::core::cloud_llm::CloudClient::new(
                &backend.base_url,
                &backend.api_key,
                &backend.model,
            );
            let cancel_key = request_key.clone();
            let is_cancelled = move || crate::commands::opencode::is_cancelled(&cancel_key);

            let messages = cloud_messages(&parts);
            let run = client.chat_streaming(&messages, &mut on_delta, &is_cancelled);
            let budget = crate::commands::opencode::PromptBudget::CHAT;
            let result = match tokio::time::timeout(budget.total, run).await {
                Ok(result) => result,
                Err(_elapsed) => Err(format!(
                    "Bulut sağlayıcısı {} saniyede yanıt vermedi.",
                    budget.total.as_secs()
                )),
            };
            crate::commands::opencode::clear_cancelled(&request_key);

            let reply = result?;
            tracing::info!(
                "Sohbet yanıtı bulut sağlayıcısıyla üretildi ({} / {}, {} karakter)",
                backend.provider,
                backend.model,
                reply.text.chars().count()
            );
            (reply.text, None)
        }
        Some(_) => {
            backend_note = Some(
                "Bulut sağlayıcı yapılandırılmış ama İnternet modu kapalı; yerel opencode kullanılıyor."
                    .to_string(),
            );
            let (reply, used) = crate::commands::opencode::run_prompt_streaming_with_session(
                &state,
                &prompt,
                request_id.as_deref(),
                crate::commands::opencode::PromptBudget::CHAT,
                reuse_session.as_deref(),
            )
            .await?;
            (reply.text, used)
        }
        None => {
            let (reply, used) = crate::commands::opencode::run_prompt_streaming_with_session(
                &state,
                &prompt,
                request_id.as_deref(),
                crate::commands::opencode::PromptBudget::CHAT,
                reuse_session.as_deref(),
            )
            .await?;
            (reply.text, used)
        }
    };

    if notice.is_none() {
        notice = backend_note;
    }

    let public_sources: Vec<ChatSource> = sources.iter().map(source_to_public).collect();
    let sources_json = serde_json::to_string(&public_sources).unwrap_or_else(|_| "[]".to_string());

    let mut message_id = 0i64;
    match state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .append_chat_message(session_id, "assistant", &reply_text, Some(&sources_json))
    {
        Ok(id) => message_id = id,
        Err(error) => {
            // Örn. oturum bu sırada kilitlendi: yanıt yine de kullanıcıya döner.
            tracing::warn!("Asistan yanıtı kaydedilemedi: {}", error);
        }
    }

    if used_opencode_session.as_deref() != session.opencode_session_id.as_deref()
        || current_model != session.model
    {
        if let Ok(db) = state.db.read() {
            if let Err(error) = db.set_chat_session_opencode(
                session_id,
                current_model.as_deref(),
                used_opencode_session.as_deref(),
            ) {
                tracing::warn!("Sohbet oturumu opencode kimliği güncellenemedi: {}", error);
            }
        }
    }

    Ok(ChatSendResponse {
        message_id,
        reply: reply_text,
        sources: public_sources,
        notice,
    })
}

/// opencode için tek metin prompt'u (parçalardan üretilir).
fn render_opencode_prompt(parts: &ChatPromptParts) -> String {
    let mut prompt = parts.system.clone();
    prompt.push('\n');

    if !parts.history.is_empty() {
        let mut transcript = String::new();
        for (role, content) in &parts.history {
            let label = if role == "assistant" {
                "Asistan"
            } else {
                "Kullanıcı"
            };
            transcript.push_str(&format!("{}: {}\n", label, content));
        }
        prompt.push_str("KONUŞMA GEÇMİŞİ:\n");
        prompt.push_str(&crate::utils::truncate_prompt_text(
            &transcript,
            HISTORY_CHAR_CAP,
        ));
        prompt.push_str("\n\n");
    }

    prompt.push_str("KULLANICI MESAJI:\n");
    prompt.push_str(&parts.question);
    prompt
}

/// Bulut sağlayıcıları için OpenAI uyumlu mesaj dizisi.
fn cloud_messages(parts: &ChatPromptParts) -> Vec<crate::core::cloud_llm::ChatCompletionMessage> {
    let mut messages = vec![crate::core::cloud_llm::ChatCompletionMessage::system(
        parts.system.clone(),
    )];
    for (role, content) in &parts.history {
        let message = if role == "assistant" {
            crate::core::cloud_llm::ChatCompletionMessage::assistant(content.clone())
        } else {
            crate::core::cloud_llm::ChatCompletionMessage::user(content.clone())
        };
        messages.push(message);
    }
    messages.push(crate::core::cloud_llm::ChatCompletionMessage::user(
        parts.question.clone(),
    ));
    messages
}

/// Kaynak metinleri bloğunu üretir (hem opencode prompt'u hem bulut mesajı için).
fn render_sources_block(sources: &[RetrievedSource]) -> String {
    if sources.is_empty() {
        return "KAYNAK METİNLER: bu tur için yerel arşivden kaynak bulunamadı.\n".to_string();
    }

    let mut block = String::from("KAYNAK METİNLER:\n");
    for (index, source) in sources.iter().enumerate() {
        let origin = if source.kind == "web" {
            " [internet kaynağı]"
        } else {
            ""
        };
        block.push_str(&format!(
            "\n[Kaynak {}] {}{}\n{}\n",
            index + 1,
            source.label,
            origin,
            source.text
        ));
    }
    block.push('\n');
    block
}

/// Geçmiş penceresini (rol, içerik) çiftleri olarak döner.
fn history_pairs(history: &[ChatMessage]) -> Vec<(String, String)> {
    history
        .iter()
        .rev()
        .take(HISTORY_WINDOW)
        .rev()
        .map(|message| {
            let role = if message.role == "assistant" {
                "assistant".to_string()
            } else {
                "user".to_string()
            };
            (role, message.content.clone())
        })
        .collect()
}

/// Sohbet isteğinin yapısal parçaları.
pub struct ChatPromptParts {
    /// Sistem promptu + kaynaklar.
    pub system: String,
    /// Son N mesaj (rol, içerik).
    pub history: Vec<(String, String)>,
    /// Kullanıcının güncel sorusu.
    pub question: String,
}

/// Prompt parçalarını kurar (OpenAI uyumlu mesaj dizisi ve opencode metni
/// aynı içerikten türetilir).
pub fn build_chat_parts(
    history: &[ChatMessage],
    sources: &[RetrievedSource],
    question: &str,
) -> ChatPromptParts {
    let mut system = String::from(CHAT_SYSTEM_PROMPT);
    system.push_str("\n\n");
    system.push_str(&render_sources_block(sources));

    ChatPromptParts {
        system,
        history: history_pairs(history),
        question: question.to_string(),
    }
}

/// opencode için tek metin prompt'u.
pub fn build_chat_prompt(
    history: &[ChatMessage],
    sources: &[RetrievedSource],
    question: &str,
) -> String {
    let parts = build_chat_parts(history, sources, question);

    let mut prompt = parts.system.clone();
    prompt.push('\n');

    if !parts.history.is_empty() {
        let mut transcript = String::new();
        for (role, content) in &parts.history {
            let label = if role == "assistant" {
                "Asistan"
            } else {
                "Kullanıcı"
            };
            transcript.push_str(&format!("{}: {}\n", label, content));
        }
        prompt.push_str("KONUŞMA GEÇMİŞİ:\n");
        prompt.push_str(&crate::utils::truncate_prompt_text(
            &transcript,
            HISTORY_CHAR_CAP,
        ));
        prompt.push_str("\n\n");
    }

    prompt.push_str("KULLANICI MESAJI:\n");
    prompt.push_str(&parts.question);
    prompt
}

fn source_to_public(source: &RetrievedSource) -> ChatSource {
    ChatSource {
        label: source.label.clone(),
        url: source.url.clone(),
        kind: source.kind.to_string(),
        score: Some(source.score),
    }
}
