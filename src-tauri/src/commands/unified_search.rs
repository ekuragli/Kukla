use tauri::State;

use crate::core::rag::hybrid::{self, bedesten_rank_score, score_to_percent};
use crate::core::yargitay::client::map_daire_code;
use crate::models::{Decision, DecisionSource, SearchResult, UnifiedSearchOutput};
use crate::AppState;

const LOCAL_CANDIDATES: u32 = 30;
const FINAL_LIMIT: usize = 20;
const YARGITAY_PAGE_SIZE: u32 = 10;
const MAX_SEARCH_PHRASE_CHARS: usize = 500;

#[tauri::command]
pub async fn search_unified(
    state: State<'_, AppState>,
    query: String,
    court_types: Option<Vec<String>>,
    daire: Option<String>,
    year_start: Option<i32>,
    year_end: Option<i32>,
    source_filter: Option<String>,
    page: Option<u32>,
) -> Result<UnifiedSearchOutput, String> {
    let page = page.unwrap_or(1).max(1);
    crate::commands::auth_guard::require_auth(&state)?;
    state.audit.log_search(&query);

    let sanitized = crate::utils::sanitize_query(&query);
    if sanitized.is_empty() {
        return Err("Sorgu boş olamaz. Lütfen dava özetinizi veya anahtar kelimeleri girin.".to_string());
    }

    if sanitized.chars().count() < 10 {
        return Err(
            "Sorgu çok kısa. Lütfen sorgunuzu genişletin. Örn: 'kira uyarlama', 'iş kazası tazminatı'"
                .to_string(),
        );
    }

    let courts = court_types.unwrap_or_else(|| vec!["YARGITAYKARARI".to_string()]);
    let search_yargitay = courts.iter().any(|c| c == "YARGITAYKARARI");
    let bedesten_courts: Vec<String> = courts
        .into_iter()
        .filter(|c| c != "YARGITAYKARARI")
        .collect();

    let mut merged: Vec<SearchResult> = Vec::new();
    let mut has_more = false;
    let has_filters = daire.is_some() || year_start.is_some() || year_end.is_some();

    // Yerel vektörel (semantic) arama — Candle + all-MiniLM-L6-v2.
    let local = local_semantic_results(&state, &sanitized, LOCAL_CANDIDATES).await;
    let use_local_semantic = !local.is_empty();
    merged.extend(local);

    let yargitay_max_pages = if page > 1 {
        1
    } else if has_filters {
        3
    } else {
        1
    };

    if search_yargitay {
        match fetch_yargitay_results(
            &state,
            &sanitized,
            daire.as_deref(),
            year_start,
            year_end,
            page,
            yargitay_max_pages,
        )
        .await
        {
            Ok((mut yargitay, more)) => {
                merged.append(&mut yargitay);
                has_more = has_more || more;
            }
            Err(e) => {
                tracing::error!("Yargıtay araması başarısız: {}", e);
                if bedesten_courts.is_empty() {
                    return Err(e);
                }
            }
        }
    }

    if !bedesten_courts.is_empty() {
        match fetch_bedesten_results(
            &state,
            &sanitized,
            Some(bedesten_courts),
            daire.as_deref(),
            year_start,
            year_end,
            page,
        )
        .await
        {
            Ok((mut online, more)) => {
                merged.append(&mut online);
                has_more = has_more || more;
            }
            Err(e) => {
                tracing::error!("Bedesten araması başarısız: {}", e);
                if merged.is_empty() {
                    return Err(e);
                }
            }
        }
    }

    apply_source_filter(&mut merged, source_filter.as_deref());
    dedupe_results(&mut merged);

    // Yerel + çevrimiçi sonuçlar birleştikten sonra liste sınırlanır; kırpılan
    // sonuçlar için "daha fazla" bilgisi korunur.
    if merged.len() > FINAL_LIMIT {
        has_more = true;
        merged.truncate(FINAL_LIMIT);
    }

    let search_mode = if use_local_semantic {
        "hybrid_semantic"
    } else {
        "online_keyword"
    }
    .to_string();

    Ok(UnifiedSearchOutput {
        results: merged,
        page,
        has_more,
        search_mode,
    })
}

/// Sorguyu yerel model ile gömüp vector store üzerinde arar.
///
/// Model dosyaları indirilmemişse arama sessizce atlanır (çevrimiçi sonuçlar
/// etkilenmez); indirme yalnızca belge indekslerken veya Ayarlar'dan yapılır.
async fn local_semantic_results(
    state: &AppState,
    query: &str,
    top_k: u32,
) -> Vec<SearchResult> {
    let embedder = match crate::commands::embedding::ensure_embedding_cached(state).await {
        Ok(embedder) => embedder,
        Err(error) => {
            tracing::debug!("Yerel semantic arama atlandı: {}", error);
            return Vec::new();
        }
    };

    let query_text = query.to_string();
    let vector = match tokio::task::spawn_blocking(move || embedder.embed(&query_text)).await {
        Ok(Ok(vector)) => vector,
        Ok(Err(error)) => {
            tracing::warn!("Sorgu embedding'i üretilemedi: {}", error);
            return Vec::new();
        }
        Err(error) => {
            tracing::warn!("Sorgu embedding görevi başarısız: {}", error);
            return Vec::new();
        }
    };

    let hits = match state
        .vector_db
        .read()
        .map_err(|_| "Vector store kilitli.".to_string())
    {
        Ok(db) => db.search(&vector, top_k, None).unwrap_or_default(),
        Err(error) => {
            tracing::warn!("{}", error);
            return Vec::new();
        }
    };

    if hits.is_empty() {
        return Vec::new();
    }

    let db = match state.db.read() {
        Ok(db) => db,
        Err(_) => return Vec::new(),
    };

    hits.into_iter()
        .filter_map(|hit| hit_to_search_result(&db, hit))
        .collect()
}

fn hit_to_search_result(
    db: &crate::db::sqlite::MetadataDb,
    hit: crate::core::rag::vector_search::SearchHit,
) -> Option<SearchResult> {
    let meta = hit.metadata;
    let decision_id = crate::utils::chunk_meta::decision_id_from_chunk(&meta).unwrap_or(0);

    let decision = match db.get_decision(decision_id).ok().flatten() {
        Some(mut decision) => {
            decision.similarity_score = Some(hit.score);
            decision
        }
        None => Decision {
            id: decision_id,
            case_id: 0,
            source: if meta.source == "user_uploaded" {
                DecisionSource::UserUploaded
            } else {
                DecisionSource::Yargitay
            },
            esas_no: meta.esas_no.clone(),
            karar_no: meta.karar_no.clone(),
            karar_tarihi: meta.karar_tarihi,
            daire: meta.daire.clone(),
            summary: Some(crate::utils::truncate_text(&meta.text_chunk, 300)),
            ratio_decidendi: None,
            similarity_score: Some(hit.score),
            created_at: chrono::Utc::now(),
        },
    };

    Some(SearchResult {
        similarity_percent: score_to_percent(hit.score),
        // Sonuç listesinde 3 satır gösterilir; özet/dilekçe için tüm parça metni
        // snippet içinde taşınır.
        snippet: meta.text_chunk.clone(),
        decision,
        online_source: None,
    })
}

fn clip_search_phrase(query: &str, max_chars: usize) -> String {
    if query.chars().count() <= max_chars {
        query.to_string()
    } else {
        query.chars().take(max_chars).collect()
    }
}

fn online_priority(result: &SearchResult) -> u8 {
    match result.online_source.as_deref() {
        Some("yargitay") => 2,
        Some("bedesten") => 1,
        _ => 0,
    }
}

fn decision_matches_daire_filter(
    decision_daire: Option<&str>,
    filter_daire: Option<&str>,
) -> bool {
    let Some(expected) = filter_daire else {
        return true;
    };
    decision_daire.is_some_and(|d| d == expected)
}

async fn fetch_yargitay_results(
    state: &AppState,
    query: &str,
    daire: Option<&str>,
    year_start: Option<i32>,
    year_end: Option<i32>,
    page: u32,
    max_pages: u32,
) -> Result<(Vec<SearchResult>, bool), String> {
    let (hukuk, ceza, kurul) = daire
        .map(map_daire_code)
        .unwrap_or((None, None, None));
    let expected_daire = hukuk
        .as_ref()
        .or(ceza.as_ref())
        .or(kurul.as_ref())
        .cloned()
        .or_else(|| daire.map(str::to_string));

    let search_phrase = clip_search_phrase(query.trim(), MAX_SEARCH_PHRASE_CHARS);

    let yargitay = state
        .yargitay_client
        .search_decisions(
            &search_phrase,
            hukuk.as_deref(),
            ceza.as_deref(),
            kurul.as_deref(),
            year_start,
            year_end,
            page,
            YARGITAY_PAGE_SIZE,
            max_pages,
        )
        .await
        .map_err(|e| format!("Yargıtay arama hatası: {}", e))?;

    let has_more = yargitay.total_records > (page as i64 + max_pages as i64 - 1) * YARGITAY_PAGE_SIZE as i64;

    let mut results = Vec::new();
    for (index, d) in yargitay.decisions.iter().enumerate() {
        if !decision_matches_daire_filter(d.daire.as_deref(), expected_daire.as_deref()) {
            continue;
        }

        let karar_tarihi = d
            .kararTarihi
            .as_deref()
            .and_then(crate::commands::bedesten_search::parse_turkish_date);

        if !crate::commands::bedesten_search::decision_matches_year_range(
            karar_tarihi,
            year_start,
            year_end,
        ) {
            continue;
        }

        let rank_score = bedesten_rank_score(index);

        let decision = Decision {
            id: d.id.parse().unwrap_or(0),
            case_id: 0,
            source: DecisionSource::Yargitay,
            esas_no: d.esasNo.clone(),
            karar_no: d.kararNo.clone(),
            karar_tarihi,
            daire: d.daire.clone(),
            summary: None,
            ratio_decidendi: None,
            similarity_score: Some(rank_score),
            created_at: chrono::Utc::now(),
        };

        results.push(SearchResult {
            similarity_percent: score_to_percent(rank_score),
            snippet: d
                .arananKelime
                .as_deref()
                .map(|s| crate::utils::truncate_text(s, 200))
                .unwrap_or_default(),
            decision,
            online_source: Some("yargitay".to_string()),
        });
    }

    Ok((results, has_more))
}

async fn fetch_bedesten_results(
    state: &AppState,
    query: &str,
    court_types: Option<Vec<String>>,
    daire: Option<&str>,
    year_start: Option<i32>,
    year_end: Option<i32>,
    page: u32,
) -> Result<(Vec<SearchResult>, bool), String> {
    let bedesten = state
        .bedesten_client
        .search_documents(query, court_types, daire, year_start, year_end, page)
        .await
        .map_err(|e| format!("Bedesten arama hatası: {}", e))?;

    let has_more = bedesten.totalRecords > page as i64 * 10;

    let mut results = Vec::new();
    for (index, d) in bedesten.decisions.iter().enumerate() {
        let source = match d.itemType.as_ref().map(|t| t.name.as_str()) {
            Some("DANISTAYKARAR") => DecisionSource::Danistay,
            Some("YERELHUKUK") | Some("ISTINAFHUKUK") => DecisionSource::Bam,
            _ => DecisionSource::Yargitay,
        };

        let karar_tarihi = d
            .kararTarihiStr
            .as_deref()
            .and_then(crate::commands::bedesten_search::parse_turkish_date);

        if !crate::commands::bedesten_search::decision_matches_year_range(
            karar_tarihi,
            year_start,
            year_end,
        ) {
            continue;
        }

        let rank_score = bedesten_rank_score(index);
        let metadata_boost = hybrid::keyword_score_for_fields(
            query,
            d.esasNo.as_deref(),
            d.kararNo.as_deref(),
            d.birimAdi.as_deref(),
            "",
        );
        let score = hybrid::hybrid_score(rank_score, metadata_boost);

        let decision = Decision {
            id: d.documentId.parse().unwrap_or(0),
            case_id: 0,
            source,
            esas_no: d.esasNo.clone(),
            karar_no: d.kararNo.clone(),
            karar_tarihi,
            daire: d.birimAdi.clone(),
            summary: None,
            ratio_decidendi: None,
            similarity_score: Some(score),
            created_at: chrono::Utc::now(),
        };

        results.push(SearchResult {
            similarity_percent: score_to_percent(score),
            snippet: String::new(),
            decision,
            online_source: Some("bedesten".to_string()),
        });
    }

    Ok((results, has_more))
}

/// Yalnızca çevrimiçi veya yalnızca yerel sonuçları filtreler.
pub fn apply_source_filter(results: &mut Vec<SearchResult>, source_filter: Option<&str>) {
    match source_filter {
        Some("online") => results.retain(|r| r.online_source.is_some()),
        Some("local") => results.retain(|r| r.online_source.is_none()),
        _ => {}
    }
}

/// Tekrarlayan kararları ayıklar ve sıralar: çevrimiçi (Yargıtay > Bedesten)
/// önce, ardından benzerlik oranına göre.
pub fn dedupe_results(results: &mut Vec<SearchResult>) {
    results.sort_by(|a, b| {
        online_priority(b)
            .cmp(&online_priority(a))
            .then_with(|| b.similarity_percent.cmp(&a.similarity_percent))
    });

    let mut seen_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut seen_refs: std::collections::HashSet<String> = std::collections::HashSet::new();

    results.retain(|r| {
        if r.decision.id > 0 {
            if seen_ids.contains(&r.decision.id) {
                return false;
            }
            seen_ids.insert(r.decision.id);
            return true;
        }

        let key = format!(
            "{}|{}|{}",
            r.decision.esas_no.as_deref().unwrap_or(""),
            r.decision.karar_no.as_deref().unwrap_or(""),
            r.decision.daire.as_deref().unwrap_or("")
        );
        if key == "||" {
            return true;
        }
        if seen_refs.contains(&key) {
            return false;
        }
        seen_refs.insert(key);
        true
    });
}