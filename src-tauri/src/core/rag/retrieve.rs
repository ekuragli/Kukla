//! Sorguyu gömüp vektör deposunu tarayan ortak yardımcılar.
//!
//! Arama (`search_unified`) ve sohbet RAG'ı aynı akışı kullanır; atıf kimliği
//! `decisions` tablosundan ve parça metaverisinden çözülür.

use crate::core::rag::vector_search::SearchHit;
use crate::models::Decision;

/// Vektör deposundan çekilmiş, taşınabilir hale getirilmiş bir kaynak.
pub struct RetrievedSource {
    pub id: String,
    pub label: String,
    pub url: Option<String>,
    /// `local` (M2'de `web` eklenir)
    pub kind: &'static str,
    pub score: f64,
    pub text: String,
}

/// Sorguyu gömüp vektör deposunda arar.
///
/// Embedding modeli hazır değilse (indirilmedi/yüklenmedi) kaynaklar sessizce
/// atlanır; sohbet genel hukuki bilgiyle devam eder.
pub async fn retrieve_sources(
    state: &crate::AppState,
    query: &str,
    top_k: u32,
    char_cap: usize,
) -> Result<Vec<RetrievedSource>, String> {
    let embedder = match crate::commands::embedding::ensure_embedding_cached(state).await {
        Ok(embedder) => embedder,
        Err(error) => {
            tracing::debug!("Yerel semantic arama atlandı: {}", error);
            return Ok(Vec::new());
        }
    };

    let query_text = query.to_string();
    let vector = match tokio::task::spawn_blocking(move || embedder.embed(&query_text)).await {
        Ok(Ok(vector)) => vector,
        Ok(Err(error)) => {
            tracing::warn!("Sorgu embedding'i üretilemedi: {}", error);
            return Ok(Vec::new());
        }
        Err(error) => {
            tracing::warn!("Sorgu embedding görevi başarısız: {}", error);
            return Ok(Vec::new());
        }
    };

    let hits = {
        let db = state
            .vector_db
            .read()
            .map_err(|_| "Vector store kilitli.".to_string())?;
        db.search(&vector, top_k, None).unwrap_or_default()
    };
    if hits.is_empty() {
        return Ok(Vec::new());
    }

    let db = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?;

    let sources = hits
        .into_iter()
        .map(|hit| resolve_source(&db, hit, char_cap))
        .collect();
    Ok(sources)
}

fn resolve_source(
    db: &crate::db::sqlite::MetadataDb,
    hit: SearchHit,
    char_cap: usize,
) -> RetrievedSource {
    let meta = hit.metadata;
    let decision = crate::utils::chunk_meta::decision_id_from_chunk(&meta)
        .and_then(|id| db.get_decision(id).ok().flatten());

    let (label, url) = source_identity(decision.as_ref(), &meta);

    RetrievedSource {
        id: hit.id,
        label,
        url,
        kind: "local",
        score: hit.score,
        text: crate::utils::truncate_text(&meta.text_chunk, char_cap),
    }
}

/// Kaynağın okunabilir atıf etiketi ve (varsa) bağlantı adresi.
fn source_identity(
    decision: Option<&Decision>,
    meta: &crate::models::ChunkMetadata,
) -> (String, Option<String>) {
    let mut parts: Vec<String> = Vec::new();
    if let Some(decision) = decision {
        if let Some(esas) = &decision.esas_no {
            parts.push(format!("Esas: {}", esas));
        }
        if let Some(karar) = &decision.karar_no {
            parts.push(format!("Karar: {}", karar));
        }
        if let Some(daire) = &decision.daire {
            parts.push(daire.clone());
        }
        if let Some(tarih) = decision.karar_tarihi {
            parts.push(crate::utils::format_date(tarih));
        }
    }

    let metadata: serde_json::Value =
        serde_json::from_str(&meta.metadata_json).unwrap_or(serde_json::Value::Null);
    let document_id = metadata
        .pointer("/document_id")
        .and_then(|value| value.as_str());
    let source_url = metadata
        .pointer("/source_url")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());

    if !parts.is_empty() {
        return (parts.join(" | "), source_url);
    }

    let label = match document_id {
        Some(id) => format!("Bedesten doküman #{}", id),
        None => {
            let decision_id = crate::utils::chunk_meta::decision_id_from_chunk(meta).unwrap_or(0);
            format!("Karar kaydı #{}", decision_id)
        }
    };

    (label, source_url)
}
