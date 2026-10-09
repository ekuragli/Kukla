use tauri::State;
use crate::AppState;
use crate::models::SearchHistory;

#[tauri::command]
pub async fn get_search_history(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<SearchHistory>, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    let limit = limit.unwrap_or(20);
    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .get_search_history(limit)
        .map_err(|e| format!("Arama geçmişi alınamadı: {}", e))
}

#[tauri::command]
pub async fn clear_search_history(state: State<'_, AppState>) -> Result<u64, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    state
        .db
        .read()
        .map_err(|_| "Veritabanı kilitli.".to_string())?
        .clear_search_history()
        .map_err(|e| format!("Arama geçmişi temizlenemedi: {}", e))
}
