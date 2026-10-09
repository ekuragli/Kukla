use kukla_lib::db::sqlite::MetadataDb;
use kukla_lib::models::{DecisionSource, NewCase, NewDecision};

#[test]
fn decision_insert_get_delete() {
    let db = MetadataDb::connect_memory().unwrap();
    let case_id = db
        .insert_case(&NewCase {
            query_text: "test sorgu".to_string(),
            pdf_path: None,
            pdf_hash: None,
        })
        .unwrap();
    let id = db
        .insert_decision(&NewDecision {
            case_id,
            source: DecisionSource::UserUploaded,
            esas_no: Some("2024/1".to_string()),
            karar_no: None,
            karar_tarihi: None,
            daire: Some("4. HD".to_string()),
            summary: Some("Test özet".to_string()),
            ratio_decidendi: None,
            similarity_score: None,
        })
        .unwrap();

    let decision = db.get_decision(id).unwrap().expect("karar bulunmalı");
    assert_eq!(decision.esas_no.as_deref(), Some("2024/1"));

    let listed = db.list_decisions_by_source("user_uploaded").unwrap();
    assert_eq!(listed.len(), 1);

    assert!(db.delete_decision(id).unwrap());
    assert!(db.get_decision(id).unwrap().is_none());
}