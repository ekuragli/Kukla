use crate::models::ChunkMetadata;

pub fn decision_id_from_chunk(meta: &ChunkMetadata) -> Option<i64> {
    serde_json::from_str::<serde_json::Value>(&meta.metadata_json)
        .ok()
        .and_then(|v| v.get("decision_id").and_then(|d| d.as_i64()))
}

pub fn document_id_from_chunk(meta: &ChunkMetadata) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(&meta.metadata_json)
        .ok()
        .and_then(|v| v.get("document_id").and_then(|d| d.as_str().map(str::to_string)))
}

pub fn build_metadata_json(
    decision_id: i64,
    extra: serde_json::Value,
) -> String {
    let mut obj = match extra {
        serde_json::Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("data".to_string(), other);
            map
        }
    };
    obj.insert(
        "decision_id".to_string(),
        serde_json::Value::Number(decision_id.into()),
    );
    serde_json::Value::Object(obj).to_string()
}

