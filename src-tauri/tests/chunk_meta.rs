use kukla_lib::models::ChunkMetadata;
use kukla_lib::utils::chunk_meta::{build_metadata_json, decision_id_from_chunk};

#[test]
fn extracts_decision_id_from_metadata_json() {
    let meta = ChunkMetadata {
        id: "chunk-1".to_string(),
        source: "user_uploaded".to_string(),
        esas_no: None,
        karar_no: None,
        karar_tarihi: None,
        daire: None,
        text_chunk: "test".to_string(),
        metadata_json: build_metadata_json(42, serde_json::json!({"source": "test"})),
    };
    assert_eq!(decision_id_from_chunk(&meta), Some(42));
}