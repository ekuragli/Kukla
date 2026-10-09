use kukla_lib::commands::websearch::chat_web_sources_with_key;
use kukla_lib::core::websearch::{
    build_request_payload, parse_search_response, tavily_error_message, SearchDepth,
    TAVILY_FREE_MONTHLY_CREDITS,
};
use kukla_lib::models::{AiSecrets, AiSecretsStatus, mask_key};

#[test]
fn request_payload_is_bounded() {
    let payload = build_request_payload("kira uyurlama", 999, SearchDepth::Basic);
    assert_eq!(payload["query"], "kira uyurlama");
    // max_results 1..=10 aralığına kırpılır.
    assert_eq!(payload["max_results"], 10);
    assert_eq!(payload["search_depth"], "basic");
    assert_eq!(payload["include_answer"], false);
}

#[test]
fn depth_maps_from_optional_string() {
    assert_eq!(
        SearchDepth::from_optional(Some("advanced")),
        SearchDepth::Advanced
    );
    assert_eq!(SearchDepth::from_optional(Some("basic")), SearchDepth::Basic);
    assert_eq!(SearchDepth::from_optional(None), SearchDepth::Basic);
}

#[test]
fn parses_tavily_response_and_skips_url_less_items() {
    let body = r#"{
        "results": [
            {"title": "Yargıtay kararı", "url": "https://example.com/1", "content": "Metin", "score": 0.91},
            {"title": "Başlıksız"},
            {"url": "https://example.com/3", "content": "Başlık yok"},
            {"title": "URL yok", "content": "Metin"}
        ],
        "answer": null
    }"#;

    let results = parse_search_response(body).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Yargıtay kararı");
    assert_eq!(results[0].url, "https://example.com/1");
    assert!((results[0].score - 0.91).abs() < 1e-6);
    // Başlıksız kayıtta başlık URL'den türetilir.
    assert_eq!(results[1].title, "https://example.com/3");
}

#[test]
fn invalid_response_is_reported() {
    assert!(parse_search_response("not-json").is_err());
}

#[test]
fn error_messages_are_turkish_and_mention_limits() {
    assert!(tavily_error_message(401, "unauthorized").contains("anahtarı"));
    assert!(tavily_error_message(403, "forbidden").contains("anahtarı"));
    let limited = tavily_error_message(429, "too many requests");
    assert!(limited.contains("kredi"));
    assert!(limited.contains(&TAVILY_FREE_MONTHLY_CREDITS.to_string()));
    assert!(tavily_error_message(500, "boom").contains("500"));
}

#[test]
fn secrets_status_hides_key_and_masks_hint() {
    let with_key = AiSecrets {
        web_search_api_key: Some("tvly-AbCdEf123456".to_string()),
        cloud_api_key: None,
    };
    let status = AiSecretsStatus::from_secrets(&with_key);
    assert!(status.has_web_search_key);
    assert_eq!(status.web_search_key_hint.as_deref(), Some("••••3456"));
    // Durum yanıtı anahtarın kendisini içermez.
    let serialized = serde_json::to_string(&status).unwrap();
    assert!(!serialized.contains("tvly-AbCdEf123456"));

    let without_key = AiSecretsStatus::from_secrets(&AiSecrets::default());
    assert!(!without_key.has_web_search_key);
    assert!(without_key.web_search_key_hint.is_none());

    // 4 ve daha kısa anahtarlar tamamen maskelenir.
    assert_eq!(mask_key("abc"), "••••");
    assert_eq!(mask_key("abcd"), "••••");
    assert_eq!(mask_key("abcde"), "••••bcde");
    assert_eq!(mask_key(" tvly-AbCdEf123456 "), "••••3456");
}

#[tokio::test]
async fn web_sources_report_missing_key_as_notice() {
    // Anahtar yoksa kaynak listesi boş döner ve kullanıcıya not üretilir.
    let (sources, notice) = chat_web_sources_with_key(None, "kira uyurlama").await;
    assert!(sources.is_empty());
    assert!(notice.is_some_and(|value| value.contains("Tavily anahtarı")));

    // Boş/whitespace anahtarı da "yok" sayılır.
    let (sources, notice) = chat_web_sources_with_key(Some("   "), "soru").await;
    assert!(sources.is_empty());
    assert!(notice.is_some());
}
