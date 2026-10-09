use chrono::Utc;
use kukla_lib::commands::chat::build_chat_prompt;
use kukla_lib::core::rag::retrieve::RetrievedSource;
use kukla_lib::models::ChatMessage;

fn message(role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        id: 1,
        session_id: 1,
        role: role.to_string(),
        content: content.to_string(),
        sources_json: None,
        created_at: Utc::now(),
    }
}

fn source(label: &str, text: &str) -> RetrievedSource {
    RetrievedSource {
        id: "decision_1_chunk_0".to_string(),
        label: label.to_string(),
        url: None,
        kind: "local",
        score: 0.82,
        text: text.to_string(),
    }
}

#[test]
fn prompt_contains_sources_history_and_question() {
    let history = vec![message("user", "Önceki soru"), message("assistant", "Önceki yanıt")];
    let sources = vec![source("Esas: 2026/100 | 9. Hukuk", "Kira uyarlması ...")];

    let prompt = build_chat_prompt(&history, &sources, "Peki ya tahliye?");

    assert!(prompt.contains("KAYNAK METİNLER:"));
    assert!(prompt.contains("[Kaynak 1]"));
    assert!(prompt.contains("Esas: 2026/100 | 9. Hukuk"));
    assert!(prompt.contains("KONUŞMA GEÇMİŞİ:"));
    assert!(prompt.contains("Kullanıcı: Önceki soru"));
    assert!(prompt.contains("Asistan: Önceki yanıt"));
    assert!(prompt.contains("KULLANICI MESAJI:"));
    assert!(prompt.ends_with("Peki ya tahliye?"));
}

#[test]
fn prompt_notes_missing_sources() {
    let prompt = build_chat_prompt(&[], &[], "Merhaba");

    assert!(prompt.contains("KAYNAK METİNLER: bu tur için yerel arşivden kaynak bulunamadı"));
    assert!(prompt.contains("KULLANICI MESAJI:\nMerhaba"));
    assert!(!prompt.contains("KONUŞMA GEÇMİŞİ:"));
}

#[test]
fn prompt_window_keeps_last_messages_and_trims_bulky_history() {
    // 12 mesajlık geçmiş: yalnızca son 8'i girer.
    let mut history = Vec::new();
    for index in 0..12 {
        let role = if index % 2 == 0 { "user" } else { "assistant" };
        history.push(message(role, &format!("mesaj-{}", index)));
    }

    let prompt = build_chat_prompt(&history, &[], "soru");

    assert!(!prompt.contains("mesaj-3\n"));
    assert!(prompt.contains("mesaj-11"));
}

#[test]
fn prompt_instructions_stay_in_turkish_and_grounded() {
    let prompt = build_chat_prompt(&[], &[], "soru");

    assert!(prompt.contains("TÜRKÇE"));
    assert!(prompt.contains("[Kaynak N]"));
    assert!(prompt.contains("hukuki danışmanlık yerine geçmez"));
}

#[test]
fn prompt_marks_web_sources_as_internet() {
    let web = RetrievedSource {
        id: "web:https://example.com/1".to_string(),
        label: "Yargıtay kararı (https://example.com/1)".to_string(),
        url: Some("https://example.com/1".to_string()),
        kind: "web",
        score: 0.9,
        text: "internet metni".to_string(),
    };

    let prompt = build_chat_prompt(&[], &[web], "soru");

    assert!(prompt.contains("[internet kaynağı]"));
    assert!(prompt.contains("Yargıtay kararı (https://example.com/1)"));
    assert!(prompt.contains("internet metni"));
}
