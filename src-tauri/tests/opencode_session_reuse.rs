//! opencode oturum sürekliliği regresyon testleri (mock HTTP sunucusu).
//!
//! Gerçek hata: var olan oturuma ikinci prompt gönderildiğinde yoklama
//! döngüsü, önceki turun TAMAMLANMIŞ asistan mesajını anında "yeni yanıt"
//! olarak okuyordu; sohbet her mesajda bir önceki cevabı döndürüyordu.
//! Bu testler o akışı gerçek HTTP üzerinde canlı tutar.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use kukla_lib::core::opencode_client::{OpencodeClient, PromptHooks};

/// Mock sunucunun durumu: oturumdaki mesaj listesi, sayaç ve bekleyen yanıt.
#[derive(Default)]
struct MockState {
    messages: Vec<serde_json::Value>,
    next_id: u32,
    /// Prompt alındığında yanıt hemen üretilmez; bu gecikmeyle eklenir
    /// (gerçek sunucudaki üretim süresini taklit eder — hata bu pencerede çıkar).
    pending_reply: Option<String>,
    prompt_at: Option<std::time::Instant>,
}

/// Model yanıtı bu süre sonra "üretilir" (yoklama aralığı 400ms).
const GENERATION_DELAY: Duration = Duration::from_millis(700);

impl MockState {
    fn push_message(&mut self, role: &str, text: &str, completed: bool) -> String {
        self.next_id += 1;
        let id = format!("msg_{}", self.next_id);
        let created = 1_000 + self.next_id as i64 * 100;
        let time = if completed {
            json!({ "created": created, "completed": created + 500 })
        } else {
            json!({ "created": created })
        };
        self.messages.push(json!({
            "id": id,
            "type": role,
            "role": role,
            "content": [{ "type": "text", "text": text }],
            "time": time,
        }));
        id
    }

    /// Bekleyen yanıt varsa ve gecikme geçtiyse mesaj listesine ekler.
    fn flush_pending_reply(&mut self) {
        if let (Some(reply), Some(at)) = (self.pending_reply.take(), self.prompt_at) {
            if at.elapsed() >= GENERATION_DELAY {
                self.push_message("assistant", &reply, true);
            } else {
                // Süre dolmadı: beklemede kalır.
                self.pending_reply = Some(reply);
            }
        }
    }
}

/// opencode HTTP API'sinin tamamını taklit eden mock sunucu kurar.
async fn mock_opencode(state: Arc<Mutex<MockState>>) -> MockServer {
    let server = MockServer::start().await;
    let base = json!({ "healthy": true });

    Mock::given(method("GET"))
        .and(path("/api/health"))
        .respond_with({
            let base = base.clone();
            move |_req: &wiremock::Request| ResponseTemplate::new(200).set_body_json(base.clone())
        })
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "id": "ses_test" }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/session/ses_test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "id": "ses_test",
                "model": { "providerID": "opencode", "id": "space-bunny-free" }
            }
        })))
        .mount(&server)
        .await;

    // Yetkilendirme bekleyen araç isteği yok.
    Mock::given(method("GET"))
        .and(path("/api/session/ses_test/permission"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [] })))
        .mount(&server)
        .await;

    // Prompt: kullanıcı mesajı + yanıt ekler (yanıt hemen tamamlanmış).
    let prompt_state = Arc::clone(&state);
    Mock::given(method("POST"))
        .and(path("/api/session/ses_test/prompt"))
        .respond_with(move |req: &wiremock::Request| {
            let body: serde_json::Value =
                serde_json::from_slice(&req.body).unwrap_or(json!({}));
            let question = body
                .pointer("/prompt/text")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
           
            let mut state = prompt_state.lock().unwrap();
            // Cevap sırası: 1. "YANIT-A", 2. "YANIT-B", 3. "YANIT-C"...
            let index = state
                .messages
                .iter()
                .filter(|message| message.get("type") == Some(&json!("assistant")))
                .count();
            let reply = format!("YANIT-{}", (b'A' + index as u8) as char);
            state.push_message("user", &question, true);
            // Yanıt GECİKMELİ üretilir: kullanıcı mesajı hemen, yanıt sonra.
            state.pending_reply = Some(reply.clone());
            state.prompt_at = Some(std::time::Instant::now());
           
            ResponseTemplate::new(200).set_body_json(json!({ "ok": true }))
        })
        .mount(&server)
        .await;

    // Mesaj listesi (sıralı). Bekleyen yanıtın süresi dolduysa burada eklenir.
    let messages_state = Arc::clone(&state);
    Mock::given(method("GET"))
        .and(path("/api/session/ses_test/message"))
        .and(query_param("order", "asc"))
        .respond_with(move |_req: &wiremock::Request| {
            let mut state = messages_state.lock().unwrap();
            state.flush_pending_reply();
            ResponseTemplate::new(200).set_body_json(json!({ "data": state.messages }))
        })
        .mount(&server)
        .await;

    server
}

/// KRİTİK REGRESYON: var olan oturumdaki ikinci istek, önceki turun
/// yanıtını değil yeni yanıtı döndürmeli.
#[tokio::test]
async fn reused_session_returns_new_reply_not_previous() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let server = mock_opencode(Arc::clone(&state)).await;
    let client = OpencodeClient::new(server.uri());

    // 1) Yeni oturumda ilk istek.
    let first = client
        .chat("Birinci soru", None)
        .await
        .expect("ilk yanıt alınmalı");
    assert_eq!(first.text, "YANIT-A");

    // 2) AYNI oturumda ikinci istek.
    let second = client
        .chat_with_hooks("İkinci soru", None, &PromptHooks::NONE)
        .await
        .expect("ikinci yanıt alınmalı");
    assert_eq!(
        second.text, "YANIT-B",
        "oturum yeniden kullanıldığında önceki yanıt dönmemeli"
    );

    // 3) Üçüncü istek: yine yeni yanıt.
    let third = client
        .chat_with_hooks("Üçüncü soru", None, &PromptHooks::NONE)
        .await
        .expect("üçüncü yanıt alınmalı");
    assert_eq!(third.text, "YANIT-C");
}

/// Üretim önizlemesi de yeni mesajı göstermeli (eski yanıtı değil).
#[tokio::test]
async fn progress_preview_shows_new_message_only() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let server = mock_opencode(Arc::clone(&state)).await;
    let client = OpencodeClient::new(server.uri());

    // İlk yanıt (oturumu oluşturur).
    let _ = client.chat("Birinci", None).await.expect("ilk yanıt");

    // İkinci istek: önizleme olarak YANIT-B akmalı, YANIT-A değil.
    let previews = Arc::new(Mutex::new(Vec::new()));
    let preview_sink = Arc::clone(&previews);
    let on_progress = move |_session: &str, text: &str| {
        preview_sink.lock().unwrap().push(text.to_string());
    };
    let hooks = PromptHooks {
        on_progress: Some(&on_progress),
        is_cancelled: None,
    };
    let reply = client
        .chat_with_hooks("İkinci", None, &hooks)
        .await
        .expect("ikinci yanıt");
    assert_eq!(reply.text, "YANIT-B");

    let previews = previews.lock().unwrap();
    assert!(
        previews.iter().all(|text| !text.contains("YANIT-A")),
        "önizlemede eski yanıt görünmemeli: {:?}",
        previews
    );
    assert!(
        previews.iter().any(|text| text.contains("YANIT-B")),
        "önizlemede yeni yanıt akmalı: {:?}",
        previews
    );
}

/// İptal: önceden iptal edilen istek anında "iptal edildi" hatası dönmeli.
#[tokio::test]
async fn pre_cancelled_request_returns_immediately() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let server = mock_opencode(Arc::clone(&state)).await;
    let client = OpencodeClient::new(server.uri());

    let _ = client.chat("Birinci", None).await.expect("ilk yanıt");

    let pre_cancelled = || true;
    let hooks = PromptHooks {
        on_progress: None,
        is_cancelled: Some(&pre_cancelled),
    };
    let started = std::time::Instant::now();
    let result = client
        .chat_with_hooks("İptal edilmeli", None, &hooks)
        .await;
    let elapsed = started.elapsed();

    assert!(result.is_err(), "iptal edilen istek yanıt dönmemeli");
    assert!(result.unwrap_err().contains("iptal"));
    assert!(
        elapsed < Duration::from_secs(2),
        "iptal anında dönmeliydi, {} sn sürdü",
        elapsed.as_secs()
    );
}

/// Sunucu sağlıksızsa hata anlaşılır olmalı.
#[tokio::test]
async fn unreachable_server_reports_connection_error() {
    // Kapalı bir port: bağlantı reddedilir (mock sunucuya ihtiyaç yok).
    let client = OpencodeClient::new("http://127.0.0.1:1");

    let result = client.chat("Merhaba", None).await;
    assert!(result.is_err());
    let message = result.unwrap_err();
    assert!(
        message.contains("ulaşılamadı") || message.contains("bağlantı"),
        "bağlantı hatası Türkçe ve anlaşılır olmalı: {}",
        message
    );
}
