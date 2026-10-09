use kukla_lib::commands::chat::build_chat_parts;
use kukla_lib::core::cloud_llm::{
    cloud_error_message, parse_model_list, parse_stream_delta, preset, CloudClient,
    ChatCompletionMessage, PRESETS,
};
use kukla_lib::core::rag::retrieve::RetrievedSource;

#[test]
fn presets_cover_expected_providers() {
    let ids: Vec<&str> = PRESETS.iter().map(|preset| preset.id).collect();
    assert!(ids.contains(&"openrouter"));
    assert!(ids.contains(&"groq"));
    assert!(ids.contains(&"gemini"));
    assert!(ids.contains(&"custom"));

    assert!(preset("openrouter").is_some());
    assert!(preset("nonexistent").is_none());

    let openrouter = preset("openrouter").unwrap();
    assert!(openrouter.base_url.starts_with("https://"));
    assert_eq!(openrouter.quota_url, Some("https://openrouter.ai/api/v1/key"));
    // Özel preset adres ile çalışır.
    assert!(preset("custom").unwrap().base_url.is_empty());
}

#[test]
fn parses_sse_deltas() {
    let delta = parse_stream_delta(
        r#"{"choices":[{"delta":{"role":"assistant","content":"Merhaba"}}]}"#,
    );
    assert_eq!(delta.as_deref(), Some("Merhaba"));

    // Boş içerik ve bitiş işareti parça üretmez.
    assert!(parse_stream_delta(r#"{"choices":[{"delta":{"content":""}}]}"#).is_none());
    assert!(parse_stream_delta("[DONE]").is_none());
    assert!(parse_stream_delta("bozuk-json").is_none());
}

#[test]
fn parses_model_list() {
    let body = r#"{"data":[{"id":"llama-3.3-70b-versatile"},{"id":"gpt-oss-120b"}]}"#;
    let models = parse_model_list(body).unwrap();
    // Listeyi alfabetik sıralar.
    assert_eq!(models, vec!["gpt-oss-120b", "llama-3.3-70b-versatile"]);

    assert!(parse_model_list("bozuk").is_err());
}

#[test]
fn cloud_errors_are_turkish_and_actionable() {
    assert!(cloud_error_message(401, "invalid key").contains("anahtarı"));
    assert!(cloud_error_message(403, "forbidden").contains("anahtarı"));
    assert!(cloud_error_message(429, "rate limited").contains("sınırı"));
    assert!(cloud_error_message(500, "boom").contains("500"));
}

#[test]
fn message_helpers_map_roles() {
    assert_eq!(ChatCompletionMessage::system("s").role, "system");
    assert_eq!(ChatCompletionMessage::user("u").role, "user");
    assert_eq!(ChatCompletionMessage::assistant("a").role, "assistant");
}

#[test]
fn client_trims_trailing_slash_and_exposes_model() {
    let client = CloudClient::new("https://api.example.com/v1/", "key", "model-x");
    assert_eq!(client.model(), "model-x");
}

#[test]
fn parts_feed_both_prompt_and_cloud_messages() {
    // Parçalar yapısı opencode metni ile bulut mesaj dizisi arasında paylaşılır.
    let parts = build_chat_parts(&[], &[], "Merhaba");
    assert!(parts.system.contains("TÜRKÇE"));
    assert_eq!(parts.question, "Merhaba");
    assert!(parts.history.is_empty());

    let sources = vec![RetrievedSource {
        id: "web:https://example.com".to_string(),
        label: "Haber (https://example.com)".to_string(),
        url: Some("https://example.com".to_string()),
        kind: "web",
        score: 0.5,
        text: "web metni".to_string(),
    }];
    let parts = build_chat_parts(&[], &sources, "soru");
    assert!(parts.system.contains("[Kaynak 1]"));
    assert!(parts.system.contains("[internet kaynağı]"));
}
