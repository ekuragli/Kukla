use tauri::State;

use crate::models::{AuditEventType, ChunkMetadata, DecisionSource, NewDecision};
use crate::AppState;

/// Çevrimiçi dokümanların vektör indeksini temizler (`bedesten_online`).
///
/// Eski sürümlerde indekslenen parçalar esas/karar numarası taşımıyor; bu
/// kayıtlar silinmezse sohbet/RAG atıfları kimliksiz kalır. Dokümanlar
/// kullanıcı yeniden açtığında atıf kimliğiyle yeniden indekslenir.
#[tauri::command]
pub async fn clear_online_index_command(
    state: State<'_, AppState>,
) -> Result<u64, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    clear_online_index(&state).await
}

/// İndekslenecek kararın atıf kimliği.
///
/// Çevrimiçi dokümanlarda (Bedesten/Yargıtay) bu bilgi arama sonucundan gelir;
/// frontend dokümanı çekerken iletir. PDF yüklemelerinde bilinmez.
#[derive(Debug, Clone, Default)]
pub struct DecisionCitation {
    pub esas_no: Option<String>,
    pub karar_no: Option<String>,
    pub daire: Option<String>,
    pub karar_tarihi: Option<chrono::NaiveDate>,
}

/// Atıf alanlarını parça metaverisine kopyalar (sohbet kaynak atıfları için).
pub fn apply_citation(meta: &mut ChunkMetadata, citation: &DecisionCitation) {
    meta.esas_no = citation.esas_no.clone();
    meta.karar_no = citation.karar_no.clone();
    meta.daire = citation.daire.clone();
    meta.karar_tarihi = citation.karar_tarihi;
}

/// Karar metnini parçalara böyüp embedding üretir ve vector store'a yazar.
///
/// Embedding modeli hazır değilse (indirilemedi/yüklenemedi) indeksleme
/// atlanır; karar kaydı yine de oluşturulur.
async fn insert_chunks_for_decision(
    state: &AppState,
    decision_id: i64,
    text: &str,
    vector_source: &str,
    citation: &DecisionCitation,
    extra_metadata: serde_json::Value,
) -> Result<u32, String> {
    let chunks = crate::core::rag::chunking::chunk_text(text);
    if chunks.is_empty() {
        return Err("İndekslenecek metin bulunamadı.".to_string());
    }

    let embedder = match crate::commands::embedding::ensure_embedding(state).await {
        Ok(embedder) => embedder,
        Err(error) => {
            tracing::warn!(
                "Decision {}: embedding atlandı, vektörel indeks yazılmadı: {}",
                decision_id,
                error
            );
            return Ok(chunks.len() as u32);
        }
    };

    let metadata_json = crate::utils::chunk_meta::build_metadata_json(decision_id, extra_metadata);

    let texts = chunks.clone();
    let vectors = tokio::task::spawn_blocking(move || embedder.embed_batch(&texts))
        .await
        .map_err(|e| format!("Embedding görevi başarısız: {}", e))??;

    let vector_db = state
        .vector_db
        .read()
        .map_err(|_| "Vector store kilitli.".to_string())?;

    let mut indexed = 0u32;
    for (index, (chunk, embedding)) in chunks.iter().zip(vectors.iter()).enumerate() {
        let chunk_id = format!("decision_{}_chunk_{}", decision_id, index);
        let mut metadata = ChunkMetadata {
            id: chunk_id.clone(),
            source: vector_source.to_string(),
            esas_no: None,
            karar_no: None,
            karar_tarihi: None,
            daire: None,
            text_chunk: chunk.clone(),
            metadata_json: metadata_json.clone(),
        };
        apply_citation(&mut metadata, citation);
        if let Err(error) = vector_db.insert_chunk(&chunk_id, embedding, &metadata) {
            tracing::warn!("Chunk kaydedilemedi ({}): {}", chunk_id, error);
            continue;
        }
        indexed += 1;
    }

    tracing::info!(
        "Decision {} indekslendi: {}/{} chunk",
        decision_id,
        indexed,
        chunks.len()
    );
    Ok(chunks.len() as u32)
}

/// Çevrimiçi dokümanların vektör indeksini temizler (`bedesten_online`).
///
/// Eski sürümlerde indekslenen parçalar esas/karar numarası taşımıyor; bu
/// kayıtlar silinmezse sohbet/RAG atıfları kimliksiz kalır. Dokümanlar
/// kullanıcı yeniden açtığında atıf kimliğiyle yeniden indekslenir.
pub async fn clear_online_index(state: &AppState) -> Result<u64, String> {
    let removed = state
        .vector_db
        .read()
        .map_err(|_| "Vector store kilitli.".to_string())?
        .delete_by_source("bedesten_online")
        .map_err(|e| format!("Çevrimiçi indeks temizlenemedi: {}", e))?;

    if removed > 0 {
        tracing::info!("Çevrimiçi arşiv indeksinden {} parça temizlendi", removed);
    }
    state.audit.log(
        AuditEventType::PersonalArchiveDelete,
        Some(&format!("online_index_purged_{}", removed)),
    );
    Ok(removed)
}

pub async fn index_user_upload(
    state: &AppState,
    text: &str,
    file_name: &str,
    index_kind: &str,
    audit_event: Option<(AuditEventType, String)>,
) -> Result<i64, String> {
    let decision_id = state.db.read().map_err(|_| "Veritabanı kilitli.")?.
        insert_decision(&NewDecision {
            case_id: 0,
            source: DecisionSource::UserUploaded,
            esas_no: None,
            karar_no: None,
            karar_tarihi: None,
            daire: None,
            summary: Some(crate::utils::truncate_text(text, 300)),
            ratio_decidendi: None,
            similarity_score: None,
        }).map_err(|e| format!("Karar kaydedilemedi: {}", e))?;

    let chunk_count = insert_chunks_for_decision(
        state,
        decision_id,
        text,
        "user_uploaded",
        &DecisionCitation::default(),
        serde_json::json!({
            "source": index_kind,
            "file_name": file_name,
        }),
    )
    .await?;

    tracing::info!(
        "User upload decision {} with {} chunks",
        decision_id,
        chunk_count
    );

    if let Some((event, details)) = audit_event {
        let _unused = state.db.read().map_err(|_| "Veritabanı kilitli.")?;
        state.audit.log(event, Some(&details));
    }

    Ok(decision_id)
}

pub async fn index_bedesten_document(
    state: &AppState,
    document_id: &str,
    text: &str,
    source_url: &str,
    source: DecisionSource,
    citation: &DecisionCitation,
) -> Result<i64, String> {
    let citation = citation.clone();
    let decision_id = state.db.read().map_err(|_| "Veritabanı kilitli.")?.
        insert_decision(&NewDecision {
            case_id: 0,
            source,
            esas_no: citation.esas_no.clone(),
            karar_no: citation.karar_no.clone(),
            karar_tarihi: citation.karar_tarihi,
            daire: citation.daire.clone(),
            summary: Some(crate::utils::truncate_text(text, 300)),
            ratio_decidendi: None,
            similarity_score: None,
        }).map_err(|e| format!("Karar kaydedilemedi: {}", e))?;

    insert_chunks_for_decision(
        state,
        decision_id,
        text,
        "bedesten_online",
        &citation,
        serde_json::json!({
            "document_id": document_id,
            "source_url": source_url,
            "source": "bedesten_online",
        }),
    )
    .await?;

    Ok(decision_id)
}