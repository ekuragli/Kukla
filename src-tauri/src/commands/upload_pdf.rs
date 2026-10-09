use tauri::State;
use crate::AppState;
use crate::models::PdfParseResult;

#[tauri::command]
pub async fn upload_and_index_pdf(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<PdfParseResult, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    let path = crate::utils::path_validation::validate_pdf_path(&file_path)?;

    if !crate::core::pdf::PdfProcessor::validate(&path).map_err(|e| format!("Dosya doğrulama hatası: {}", e))? {
        return Err("Geçersiz PDF dosyası. Lütfen bozuk olmayan bir PDF seçin.".to_string());
    }

    let page_count = crate::core::pdf::PdfProcessor::page_count(&path)
        .map_err(|e| format!("Sayfa sayısı okunamadı: {}", e))?;

    let mut result = crate::core::pdf::PdfProcessor::extract_text(&path)
        .map_err(|e| format!("PDF metni çıkarılamadı: {}", e))?;

    if result.text.trim().is_empty() {
        return Err("PDF'den metin çıkarılamadı. Dosyanın taranmış (scanned) bir PDF olmadığından emin olun.".to_string());
    }

    if page_count > 100 && result.warning.is_none() {
        result.warning = Some(format!("PDF {} sayfa. Çok uzun PDF'lerde performans düşebilir.", page_count));
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.pdf")
        .to_string();

    let decision_id = crate::commands::decision_index::index_user_upload(
        &state,
        &result.text,
        &file_name,
        "pdf_upload",
        None,
    )
    .await?;

    if result.warning.is_none() {
        result.warning = Some(format!("Karar indekslendi (ID: {})", decision_id));
    }

    Ok(result)
}