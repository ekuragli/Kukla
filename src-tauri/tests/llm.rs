use kukla_lib::core::rag::llm::{parse_petition_llm_response, parse_structured_llm_response};

#[test]
fn parses_structured_sections() {
    let raw = "---KISA_OZET---\nÖzet metni.\n\n---RATIO_DECIDENDI---\nGerekçe.\n\n---KAYNAK---\nEsas 2020/1";
    let parsed = parse_structured_llm_response(raw);
    assert!(parsed.summary.contains("Özet"));
    assert!(parsed.ratio_decidendi.contains("Gerekçe"));
    assert!(parsed.source_citation.contains("Esas"));
}

#[test]
fn parses_petition_sections() {
    let raw = "---DILEKCE---\nSayın Mahkemeye...\n\n---ICTIHAT_REFERANSLARI---\nYargıtay 9. HD Esas 2020/1\n\n---UYARI---\nTaslak metindir.";
    let parsed = parse_petition_llm_response(raw);
    assert!(parsed.draft.contains("Mahkemeye"));
    assert!(parsed.cited_decisions.contains("Yargıtay"));
    assert!(parsed.disclaimer.contains("Taslak"));
}