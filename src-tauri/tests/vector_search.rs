use kukla_lib::core::rag::vector_search::{InMemoryVectorSearch, VectorSearchTrait};
use kukla_lib::models::ChunkMetadata;

fn sample_metadata(id: &str, source: &str) -> ChunkMetadata {
    ChunkMetadata {
        id: id.to_string(),
        source: source.to_string(),
        esas_no: None,
        karar_no: None,
        karar_tarihi: None,
        daire: None,
        text_chunk: format!("metin {}", id),
        metadata_json: "{}".to_string(),
    }
}

#[test]
fn insert_search_and_delete_roundtrip() {
    let store = InMemoryVectorSearch::new();
    let embedding = vec![1.0, 0.0, 0.0];

    store
        .insert_chunk(
            "chunk-a",
            &embedding,
            &sample_metadata("chunk-a", "user_uploaded"),
        )
        .unwrap();

    let hits = store.search(&embedding, 5, None).unwrap();
    assert_eq!(hits.len(), 1);
    assert!((hits[0].score - 1.0).abs() < 0.001);

    let deleted = store.delete_chunk("chunk-a").unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(store.count().unwrap(), 0);
}

#[test]
fn delete_by_decision_id_removes_matching_chunks() {
    let store = InMemoryVectorSearch::new();
    let embedding = vec![0.5, 0.5, 0.0];

    let mut meta = sample_metadata("c1", "user_uploaded");
    meta.metadata_json =
        kukla_lib::utils::chunk_meta::build_metadata_json(99, serde_json::json!({}));

    store.insert_chunk("c1", &embedding, &meta).unwrap();
    let removed = store.delete_chunk_by_decision_id(99).unwrap();
    assert_eq!(removed, 1);
    assert_eq!(store.count().unwrap(), 0);
}