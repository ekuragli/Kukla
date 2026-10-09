use tauri::State;

use crate::AppState;
use crate::models::LlmResponse;

#[tauri::command]
pub async fn get_yargitay_document(
    state: State<'_, AppState>,
    document_id: String,
    esas_no: Option<String>,
    karar_no: Option<String>,
    daire: Option<String>,
    karar_tarihi: Option<String>,
) -> Result<LlmResponse, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let doc = state
        .yargitay_client
        .get_document(&document_id)
        .await
        .map_err(|e| format!("Yargıtay dökümanı alınamadı: {}", e))?;

    let citation = crate::commands::decision_index::DecisionCitation {
        esas_no,
        karar_no,
        daire,
        karar_tarihi: karar_tarihi
            .as_deref()
            .and_then(crate::commands::bedesten_search::parse_any_date),
    };

    let _ = crate::commands::decision_index::index_bedesten_document(
        &state,
        &document_id,
        &doc.text,
        &doc.source_url,
        crate::models::DecisionSource::Yargitay,
        &citation,
    )
    .await;

    Ok(LlmResponse {
        summary: doc.text,
        ratio_decidendi: String::new(),
        source_citation: doc.source_url,
        hallucination_score: 0.0,
    })
}