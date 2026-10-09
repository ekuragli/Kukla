use kukla_lib::core::rag::hybrid::keyword_overlap_score;
use kukla_lib::core::rag::llm::parse_structured_llm_response;

#[test]
fn hallucination_score_detects_low_overlap() {
    let source = "Davacı kira bedelinin uyarlanmasını talep etmiştir. Mahkeme talebi kabul etmiştir.";
    let unrelated = "Tamamen farklı bir konu hakkında uzun bir metin.";
    let score = keyword_overlap_score(unrelated, source);
    assert!(score < 0.5);
}

#[test]
fn hallucination_score_high_for_matching_text() {
    let source = "Davacı kira bedelinin uyarlanmasını talep etmiştir.";
    let summary = "Davacı kira bedelinin uyarlanmasını talep etmiştir.";
    let score = keyword_overlap_score(summary, source);
    assert!(score >= 0.7);
}

#[test]
fn structured_response_parsing_feeds_hallucination_check() {
    let raw = "---KISA_OZET---\nKira uyarlanması talebi reddedilmiştir.\n\n---RATIO_DECIDENDI---\nGerekçe metni.\n\n---KAYNAK---\nEsas 2020/1";
    let parsed = parse_structured_llm_response(raw);
    let source = "Mahkeme kira uyarlanması talebini reddetmiştir. Gerekçe metni.";
    let score = keyword_overlap_score(&parsed.summary, source);
    assert!(score > 0.0);
}
