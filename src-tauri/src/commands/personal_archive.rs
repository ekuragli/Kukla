use std::path::Path;
use tauri::State;
use crate::AppState;
use crate::models::{BulkUploadResult, PdfParseResult};

fn ensure_personal_archive_enabled(state: &AppState) -> Result<(), String> {
    let settings = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_settings()
        .map_err(|e| format!("Ayarlar okunamadı: {}", e))?;
    if !settings.personal_archive_enabled {
        return Err(
            "Kişisel arşiv kapalı. Ayarlardan etkinleştirebilirsiniz.".to_string(),
        );
    }
    Ok(())
}

async fn process_personal_decision_file(state: &AppState, file_path: &str) -> Result<PdfParseResult, String> {
    let path = crate::utils::path_validation::validate_pdf_path(file_path)?;

    if !crate::core::pdf::PdfProcessor::validate(&path)
        .map_err(|e| format!("Dosya doğrulama hatası: {}", e))?
    {
        return Err("Geçersiz PDF dosyası.".to_string());
    }

    let result = crate::core::pdf::PdfProcessor::extract_text(&path)
        .map_err(|e| format!("PDF metni çıkarılamadı: {}", e))?;

    if result.text.trim().is_empty() {
        return Err("PDF'den metin çıkarılamadı.".to_string());
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.pdf")
        .to_string();

    let decision_id = crate::commands::decision_index::index_user_upload(
        state,
        &result.text,
        &file_name,
        "personal_archive",
        Some((
            crate::models::AuditEventType::PersonalArchiveAdd,
            format!("personal_archive:{}", file_name),
        )),
    )
    .await?;

    let mut response = result;
    response.warning = Some(format!("Kişisel arşive eklendi (Karar ID: {})", decision_id));
    Ok(response)
}

#[tauri::command]
pub async fn add_personal_decision(
    state: State<'_, AppState>,
    file_path: String,
    consent_given: bool,
) -> Result<PdfParseResult, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    ensure_personal_archive_enabled(&state)?;

    if !consent_given {
        return Err("Yasal kullanım hakkına sahip olduğunuzu onaylamalısınız.".to_string());
    }

    process_personal_decision_file(&state, &file_path).await
}

#[tauri::command]
pub async fn add_personal_decisions_bulk(
    state: State<'_, AppState>,
    file_paths: Vec<String>,
    consent_given: bool,
) -> Result<BulkUploadResult, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    ensure_personal_archive_enabled(&state)?;

    if !consent_given {
        return Err("Yasal kullanım hakkına sahip olduğunuzu onaylamalısınız.".to_string());
    }

    if file_paths.is_empty() {
        return Err("Yüklenecek dosya seçilmedi.".to_string());
    }

    let mut result = BulkUploadResult {
        succeeded: 0,
        failed: 0,
        errors: Vec::new(),
    };

    for file_path in file_paths {
        let label = Path::new(&file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&file_path)
            .to_string();

        match process_personal_decision_file(&state, &file_path).await {
            Ok(_) => result.succeeded += 1,
            Err(err) => {
                result.failed += 1;
                result.errors.push(format!("{label}: {err}"));
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn list_personal_decisions(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let decisions = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .list_decisions_by_source("user_uploaded")
        .map_err(|e| format!("Kişisel kararlar alınamadı: {}", e))?;

    let chunks = state
        .vector_db
        .read()
        .map_err(|_| "Vector store kilitli.".to_string())?
        .list_chunks("user_uploaded")
        .unwrap_or_default();

    let items: Vec<serde_json::Value> = decisions
        .into_iter()
        .map(|decision| {
            let preview = chunks
                .iter()
                .find(|c| crate::utils::chunk_meta::decision_id_from_chunk(c) == Some(decision.id))
                .map(|c| crate::utils::truncate_text(&c.text_chunk, 100))
                .or_else(|| decision.summary.clone())
                .unwrap_or_default();

            serde_json::json!({
                "id": decision.id,
                "decision_id": decision.id,
                "source": "user_uploaded",
                "text_preview": preview,
                "created_at": decision.created_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(items)
}

#[tauri::command]
pub async fn delete_personal_decision(
    state: State<'_, AppState>,
    decision_id: i64,
) -> Result<bool, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let deleted_vectors = state
        .vector_db
        .read()
        .map_err(|_| "Vector store kilitli.".to_string())?
        .delete_chunk_by_decision_id(decision_id)
        .map_err(|e| format!("Embedding silinemedi: {}", e))?;

    let deleted_db = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .delete_decision(decision_id)
        .map_err(|e| format!("Karar silinemedi: {}", e))?;

    if !deleted_db && deleted_vectors == 0 {
        return Err("Karar bulunamadı.".to_string());
    }

    let _ = state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .insert_audit_log(
            crate::models::AuditEventType::PersonalArchiveDelete.as_str(),
            Some(&format!("decision_{}", decision_id)),
        );

    state.audit.log_personal_archive_delete(decision_id);

    Ok(true)
}