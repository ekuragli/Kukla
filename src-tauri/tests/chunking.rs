use kukla_lib::core::rag::chunking::chunk_text;

#[test]
fn single_short_text() {
    let chunks = chunk_text("kısa metin");
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "kısa metin");
}

#[test]
fn splits_long_text() {
    let text = "a".repeat(5000);
    let chunks = chunk_text(&text);
    assert!(chunks.len() > 1);
}