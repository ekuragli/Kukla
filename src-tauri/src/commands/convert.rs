use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::commands::export::{build_docx_bytes, build_pdf_bytes};
use crate::core::docx_read::read_docx_text;
use crate::core::pdf::PdfProcessor;
use crate::core::udf::parse_udf;
use crate::models::ExportTarget;
use crate::utils::path_validation::validate_input_path;
use crate::AppState;

const INPUT_EXTS: &[&str] = &["pdf", "docx", "txt", "md", "udf", "zip"];

#[derive(Debug, Serialize)]
pub struct ConvertResult {
    pub output_path: String,
    pub source_format: String,
    pub target_format: String,
    pub char_count: usize,
    pub warning: Option<String>,
}

/// PDF / DOCX / TXT / UDF (UYAP) dosyasını hedef biçime (txt | docx | pdf) çevirir.
#[tauri::command]
pub async fn convert_file(
    state: State<'_, AppState>,
    file_path: String,
    target_format: String,
    output_path: Option<String>,
) -> Result<ConvertResult, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let path = validate_input_path(&file_path, INPUT_EXTS)?;
    let target = crate::commands::export::parse_export_target(&target_format)?;
    let source_format = source_format(&path)?;

    let (text, warning) = extract_text(&path, source_format)?;
    if text.trim().is_empty() {
        return Err("Dosyadan metin çıkarılamadı (belge boş veya görüntü tabanlı olabilir).".to_string());
    }

    let output = resolve_output_path(&path, target, output_path.as_deref())?;
    let bytes = build_bytes(&text, target, &path)?;
    std::fs::write(&output, bytes).map_err(|e| format!("Dosya yazılamadı: {}", e))?;

    state.audit.log(
        crate::models::AuditEventType::Convert,
        Some(&format!("{}->{}", source_format, target_label(target))),
    );

    Ok(ConvertResult {
        output_path: output.to_string_lossy().to_string(),
        source_format: source_format.to_string(),
        target_format: target_label(target).to_string(),
        char_count: text.chars().count(),
        warning,
    })
}

fn source_format(path: &Path) -> Result<&'static str, String> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "pdf" => Ok("pdf"),
        "docx" => Ok("docx"),
        "txt" | "md" => Ok("txt"),
        "udf" => Ok("udf"),
        // `.udf.zip` gibi ikili uzantılı dosyalar "zip" görünür
        "zip" => Ok("udf"),
        other => Err(format!("Desteklenmeyen kaynak biçim: .{}", other)),
    }
}

fn target_label(target: ExportTarget) -> &'static str {
    match target {
        ExportTarget::Txt => "txt",
        ExportTarget::Docx => "docx",
        ExportTarget::Pdf => "pdf",
    }
}

fn extract_text(path: &Path, source: &str) -> Result<(String, Option<String>), String> {
    match source {
        "pdf" => {
            let parsed = PdfProcessor::extract_text(path)
                .map_err(|e| format!("PDF metni çıkarılamadı: {}", e))?;
            Ok((parsed.text, parsed.warning))
        }
        "docx" => {
            let text = read_docx_text(path)?;
            Ok((text, None))
        }
        "txt" => {
            let bytes =
                std::fs::read(path).map_err(|e| format!("Dosya okunamadı: {}", e))?;
            Ok((String::from_utf8_lossy(&bytes).into_owned(), None))
        }
        "udf" => {
            let doc = parse_udf(path)?;
            let warning = doc.has_signature.then(|| {
                "Belgede e-imza (sign.sgn) var. E-imza PDF'e taşınmaz; çıktıyı imzalamanız gerekir."
                    .to_string()
            });
            Ok((doc.text, warning))
        }
        other => Err(format!("Kaynak biçim desteklenmiyor: {}", other)),
    }
}

fn resolve_output_path(
    source: &Path,
    target: ExportTarget,
    requested: Option<&str>,
) -> Result<PathBuf, String> {
    let ext = target_label(target);

    if let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        let candidate = Path::new(requested);
        if candidate.extension().is_none() {
            let mut with_ext = candidate.as_os_str().to_owned();
            with_ext.push(format!(".{}", ext));
            return Ok(PathBuf::from(with_ext));
        }
        return Ok(candidate.to_path_buf());
    }

    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("belge");
    let stem: String = stem
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .take(48)
        .collect();
    let stem = if stem.is_empty() {
        "belge".to_string()
    } else {
        stem
    };

    let now = chrono::Utc::now();
    let file_name = format!(
        "{}_donustur_{}_{}.{}",
        stem,
        now.format("%Y%m%d_%H%M%S"),
        now.timestamp_subsec_millis(),
        ext
    );
    let desktop = dirs_next::desktop_dir().unwrap_or_else(|| PathBuf::from("."));
    Ok(desktop.join(file_name))
}

fn build_bytes(text: &str, target: ExportTarget, source: &Path) -> Result<Vec<u8>, String> {
    match target {
        ExportTarget::Txt => Ok(text.as_bytes().to_vec()),
        ExportTarget::Docx => {
            let title = source
                .file_stem()
                .and_then(|value| value.to_str())
                .filter(|value| !value.trim().is_empty());
            build_docx_bytes(text, title)
                .map_err(|e| format!("DOCX oluşturulamadı: {}", e))
        }
        ExportTarget::Pdf => {
            build_pdf_bytes(text).map_err(|e| format!("PDF oluşturulamadı: {}", e))
        }
    }
}
