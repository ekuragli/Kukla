use tauri::State;

use crate::models::SearchResult;
use crate::AppState;

#[tauri::command]
pub fn search_case(query: String) -> Result<Vec<SearchResult>, String> {
    let sanitized = query.trim().to_lowercase();

    if sanitized.chars().count() < 10 {
        return Err("Sorgu çok kısa. Lütfen sorgunuzu genişletin. Örn: 'kira uyarlama', 'iş kazası tazminatı'".to_string());
    }

    // Basit anahtar kelime filtresi (vektoral semantic search yok)
    let results: Vec<SearchResult> = Vec::new();

    Ok(results)
}

#[tauri::command]
pub fn unified_search(
    query: String,
    year_start: Option<i32>,
    year_end: Option<i32>,
    daire: Option<String>,
) -> Result<Vec<SearchResult>, String> {
    let sanitized = query.trim().to_lowercase();

    if sanitized.chars().count() < 10 {
        return Err("Sorgu çok kısa. Lütfen sorgunuzu genişletin. Örn: 'kira uyarlama', 'iş kazası tazminatı'".to_string());
    }

    // Basit anahtar kelime filtresi (vektoral semantic search yok)
    let results: Vec<SearchResult> = Vec::new();

    Ok(results)
}