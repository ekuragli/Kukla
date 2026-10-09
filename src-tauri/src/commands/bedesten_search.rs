use chrono::Datelike;
use tauri::State;
use crate::AppState;
use crate::models::SearchResult;
use crate::models::Decision;
use crate::models::DecisionSource;


#[tauri::command]
pub async fn search_bedesten(
    state: State<'_, AppState>,
    query: String,
    court_types: Option<Vec<String>>,
    daire: Option<String>,
    year_start: Option<i32>,
    year_end: Option<i32>,
) -> Result<Vec<SearchResult>, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    state.audit.log_search(&format!("bedesten:{}", query));

    let sanitized = crate::utils::sanitize_query(&query);
    if sanitized.is_empty() {
        return Err("Sorgu boş olamaz.".to_string());
    }

    let results = state
        .bedesten_client
        .search_documents(
            &sanitized,
            court_types,
            daire.as_deref(),
            year_start,
            year_end,
            1,
        )
        .await
        .map_err(|e| format!("Bedesten arama hatası: {}", e))?;

    if results.decisions.is_empty() {
        return Err("Bedesten'de sonuç bulunamadı.".to_string());
    }

    let mut search_results = Vec::new();
    for d in &results.decisions {
        let source = match d.itemType.as_ref().map(|t| t.name.as_str()) {
            Some("DANISTAYKARAR") => DecisionSource::Danistay,
            Some("YERELHUKUK") => DecisionSource::Bam,
            Some("ISTINAFHUKUK") => DecisionSource::Bam,
            _ => DecisionSource::Yargitay,
        };

        let karar_tarihi = d
            .kararTarihiStr
            .as_deref()
            .and_then(parse_turkish_date);

        let doc_id: i64 = d.documentId.parse().unwrap_or(0);

        let decision = Decision {
            id: doc_id,
            case_id: 0,
            source,
            esas_no: d.esasNo.clone(),
            karar_no: d.kararNo.clone(),
            karar_tarihi,
            daire: d.birimAdi.clone(),
            summary: None,
            ratio_decidendi: None,
            similarity_score: None,
            created_at: chrono::Utc::now(),
        };

        search_results.push(SearchResult {
            similarity_percent: 0,
            snippet: String::new(),
            decision,
            online_source: Some("bedesten".to_string()),
        });
    }

    Ok(search_results)
}

#[tauri::command]
pub async fn get_bedesten_document(
    state: State<'_, AppState>,
    document_id: String,
    source: Option<String>,
    esas_no: Option<String>,
    karar_no: Option<String>,
    daire: Option<String>,
    karar_tarihi: Option<String>,
) -> Result<crate::models::LlmResponse, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    let doc = state
        .bedesten_client
        .get_document_markdown(&document_id)
        .await
        .map_err(|e| format!("Döküman alınamadı: {}", e))?;

    let text = doc.markdownContent;

    let resolved_source = source
        .as_deref()
        .and_then(|s| s.parse::<DecisionSource>().ok())
        .unwrap_or(DecisionSource::Yargitay);

    // Atıf kimliği arama sonucundan (frontend) gelir; indekslenen parçalara
    // yazılır ki sohbet/RAG kaynakları doğru esas/karar numarasıyla gösterilsin.
    let citation = crate::commands::decision_index::DecisionCitation {
        esas_no,
        karar_no,
        daire,
        karar_tarihi: karar_tarihi.as_deref().and_then(parse_any_date),
    };

    let _ = crate::commands::decision_index::index_bedesten_document(
        &state,
        &document_id,
        &text,
        &doc.sourceUrl,
        resolved_source,
        &citation,
    )
    .await;

    Ok(crate::models::LlmResponse {
        summary: text,
        ratio_decidendi: String::new(),
        source_citation: doc.sourceUrl,
        hallucination_score: 0.0,
    })
}

pub fn parse_turkish_date(s: &str) -> Option<chrono::NaiveDate> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() == 3 {
        let day: u32 = parts[0].parse().ok()?;
        let month: u32 = parts[1].parse().ok()?;
        let year: i32 = parts.get(2).and_then(|y| y.parse().ok())?;
        chrono::NaiveDate::from_ymd_opt(year, month, day)
    } else {
        None
    }
}

/// Frontend'den gelen tarihi hem `GG.AA.YYYY` hem `YYYY-MM-DD` olarak kabul eder.
pub fn parse_any_date(s: &str) -> Option<chrono::NaiveDate> {
    let trimmed = s.trim();
    if let Some(date) = parse_turkish_date(trimmed) {
        return Some(date);
    }
    chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").ok()
}

pub fn decision_matches_year_range(
    date: Option<chrono::NaiveDate>,
    year_start: Option<i32>,
    year_end: Option<i32>,
) -> bool {
    if year_start.is_none() && year_end.is_none() {
        return true;
    }
    let Some(date) = date else {
        return false;
    };
    let year = date.year();
    if let Some(start) = year_start {
        if year < start {
            return false;
        }
    }
    if let Some(end) = year_end {
        if year > end {
            return false;
        }
    }
    true
}
