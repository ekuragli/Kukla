use std::fs;
use std::path::PathBuf;

use tauri::State;

use crate::AppState;

#[tauri::command]
pub async fn open_decision_in_browser(
    state: State<'_, AppState>,
    title: String,
    body: String,
    source_type: String,
    external_url: Option<String>,
) -> Result<(), String> {
    crate::commands::auth_guard::require_auth(&state)?;

    if source_type == "bedesten" {
        if let Some(url) = external_url.as_deref().filter(|u| !u.is_empty()) {
            return open::that(url).map_err(|e| format!("Bağlantı açılamadı: {}", e));
        }
    }

    let safe_title = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    let filename = if safe_title.is_empty() {
        format!("kukla_karar_{}.html", uuid::Uuid::new_v4())
    } else {
        format!("kukla_{}.html", safe_title.chars().take(60).collect::<String>())
    };

    let path: PathBuf = state.data_dir.join("tmp").join(filename);
    std::fs::create_dir_all(state.data_dir.join("tmp"))
        .map_err(|e| format!("Geçici dizin oluşturulamadı: {}", e))?;
    let escaped_title = html_escape(&title);
    let escaped_body = html_escape(&body);
    let site_link = html_escape(
        &external_url
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| "https://karararama.yargitay.gov.tr/".to_string()),
    );
    let site_label = if source_type == "yargitay" {
        "Yargıtay Karar Arama"
    } else {
        "Kaynak"
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="tr">
<head>
  <meta charset="UTF-8" />
  <title>{escaped_title}</title>
  <style>
    body {{ font-family: Georgia, 'Times New Roman', serif; max-width: 900px; margin: 2rem auto; padding: 0 1.5rem; line-height: 1.7; color: #1a1a1a; }}
    h1 {{ font-size: 1.25rem; margin-bottom: 0.5rem; }}
    .meta {{ color: #555; font-size: 0.9rem; margin-bottom: 1.5rem; }}
    pre {{ white-space: pre-wrap; word-wrap: break-word; font-family: inherit; }}
    a {{ color: #1d4ed8; }}
  </style>
</head>
<body>
  <h1>{escaped_title}</h1>
  <p class="meta"><a href="{site_link}" target="_blank" rel="noopener">{site_label}</a> · Kukla ile açıldı</p>
  <pre>{escaped_body}</pre>
</body>
</html>"#
    );

    fs::write(&path, html).map_err(|e| format!("Geçici dosya yazılamadı: {}", e))?;
    open::that(&path).map_err(|e| format!("Tarayıcıda açılamadı: {}", e))
}

pub fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}