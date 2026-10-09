use tauri::State;

use crate::AppState;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPathsInfo {
    pub data_dir: String,
    pub audit_log_path: String,
    pub database_path: String,
}

#[tauri::command]
pub fn get_app_paths(state: State<'_, AppState>) -> AppPathsInfo {
    let data_dir = state.data_dir.to_string_lossy().to_string();
    AppPathsInfo {
        audit_log_path: state
            .data_dir
            .join("audit.log.enc")
            .to_string_lossy()
            .to_string(),
        database_path: state
            .data_dir
            .join("kukla.db.enc")
            .to_string_lossy()
            .to_string(),
        data_dir,
    }
}