use kukla_lib::core::rag::hybrid::{
    hybrid_score, keyword_overlap_score, keyword_score_for_fields, rerank_hits, tokenize,
    KEYWORD_WEIGHT, VECTOR_WEIGHT,
};
use kukla_lib::core::rag::vector_search::SearchHit;
use kukla_lib::models::ChunkMetadata;

fn sample_hit(id: &str, vector_score: f64, text: &str) -> SearchHit {
    SearchHit {
        id: id.to_string(),
        score: vector_score,
        metadata: ChunkMetadata {
            id: id.to_string(),
            source: "user_uploaded".to_string(),
            esas_no: None,
            karar_no: None,
            karar_tarihi: None,
            daire: None,
            text_chunk: text.to_string(),
            metadata_json: "{}".to_string(),
        },
    }
}

#[test]
fn tokenize_splits_turkish_words() {
    let tokens = tokenize("Kira bedelinin uyarlanması");
    assert!(tokens.contains(&"kira".to_string()));
    assert!(tokens.contains(&"bedelinin".to_string()));
}

#[test]
fn keyword_overlap_prefers_matching_text() {
    let high = keyword_overlap_score("kira uyarlanması", "kira bedelinin uyarlanması davası");
    let low = keyword_overlap_score("kira uyarlanması", "iş kazası tazminatı");
    assert!(high > low);
}

#[test]
fn hybrid_score_blends_vector_and_keyword() {
    let blended = hybrid_score(0.8, 0.4);
    let expected = 0.8 * VECTOR_WEIGHT + 0.4 * KEYWORD_WEIGHT;
    assert!((blended - expected).abs() < 0.01);
}

#[test]
fn rerank_promotes_keyword_match() {
    let hits = vec![
        sample_hit("a", 0.9, "iş kazası tazminatı"),
        sample_hit("b", 0.7, "kira bedelinin uyarlanması"),
    ];
    let reranked = rerank_hits(hits, "kira uyarlanması");
    assert_eq!(reranked[0].id, "b");
}

#[test]
fn keyword_score_uses_metadata_fields() {
    let score = keyword_score_for_fields(
        "2020/1234",
        Some("2020/1234"),
        Some("2020/5678"),
        Some("4. Hukuk Dairesi"),
        "",
    );
    assert!(score > 0.0);
}