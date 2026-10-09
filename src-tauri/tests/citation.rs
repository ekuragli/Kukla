use chrono::NaiveDate;
use kukla_lib::commands::bedesten_search::parse_any_date;
use kukla_lib::commands::decision_index::{apply_citation, DecisionCitation};
use kukla_lib::models::ChunkMetadata;

fn chunk() -> ChunkMetadata {
    ChunkMetadata {
        id: "decision_1_chunk_0".to_string(),
        source: "bedesten_online".to_string(),
        esas_no: None,
        karar_no: None,
        karar_tarihi: None,
        daire: None,
        text_chunk: "metin".to_string(),
        metadata_json: r#"{"decision_id": 1}"#.to_string(),
    }
}

#[test]
fn citation_fields_are_copied_to_chunk_metadata() {
    let mut meta = chunk();
    let citation = DecisionCitation {
        esas_no: Some("2026/100".to_string()),
        karar_no: Some("5000".to_string()),
        daire: Some("9. Hukuk Dairesi".to_string()),
        karar_tarihi: NaiveDate::from_ymd_opt(2026, 3, 15),
    };

    apply_citation(&mut meta, &citation);

    assert_eq!(meta.esas_no.as_deref(), Some("2026/100"));
    assert_eq!(meta.karar_no.as_deref(), Some("5000"));
    assert_eq!(meta.daire.as_deref(), Some("9. Hukuk Dairesi"));
    assert_eq!(
        meta.karar_tarihi,
        NaiveDate::from_ymd_opt(2026, 3, 15)
    );
}

#[test]
fn empty_citation_keeps_metadata_empty() {
    let mut meta = chunk();
    apply_citation(&mut meta, &DecisionCitation::default());

    assert!(meta.esas_no.is_none());
    assert!(meta.karar_tarihi.is_none());
}

#[test]
fn parse_any_date_accepts_both_formats() {
    assert_eq!(
        parse_any_date("15.03.2026"),
        NaiveDate::from_ymd_opt(2026, 3, 15)
    );
    assert_eq!(
        parse_any_date("2026-03-15"),
        NaiveDate::from_ymd_opt(2026, 3, 15)
    );
    assert_eq!(parse_any_date("gecersiz"), None);
}
