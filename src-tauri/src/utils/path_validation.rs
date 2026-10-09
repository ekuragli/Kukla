use std::path::{Component, Path, PathBuf};

const BLOCKED_SEGMENTS: &[&str] = &[
    "windows",
    "system32",
    "syswow64",
    "program files",
    "programdata",
    "program files (x86)",
];

pub fn validate_pdf_path(path_str: &str) -> Result<PathBuf, String> {
    validate_input_path(path_str, &["pdf"])
}

/// Dosya dönüştürücü vb. için genel doğrulama: `allowed_exts` küçük harf uzantı listesi.
pub fn validate_input_path(path_str: &str, allowed_exts: &[&str]) -> Result<PathBuf, String> {
    if path_str.trim().is_empty() {
        return Err("Dosya yolu boş olamaz.".to_string());
    }

    let path = Path::new(path_str);

    if path_str.contains('\0') {
        return Err("Geçersiz dosya yolu.".to_string());
    }

    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err("Geçersiz dosya yolu: üst dizin referansı kullanılamaz.".to_string());
        }
    }

    if !path.exists() {
        return Err("Dosya bulunamadı.".to_string());
    }

    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Geçersiz dosya yolu: {}", e))?;

    if !canonical.is_file() {
        return Err("Yalnızca dosya seçilebilir.".to_string());
    }

    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if !allowed_exts.contains(&ext.as_str()) {
        return Err(format!(
            "Desteklenmeyen dosya biçimi: .{}. İzinli biçimler: {}.",
            ext,
            allowed_exts.join(", ")
        ));
    }

    if is_blocked_path(&canonical) {
        return Err("Bu konumdan dosya okunamaz.".to_string());
    }

    Ok(canonical)
}

fn is_blocked_path(path: &Path) -> bool {
    let normalized = path
        .to_string_lossy()
        .to_ascii_lowercase()
        .replace('/', "\\");

    BLOCKED_SEGMENTS
        .iter()
        .any(|segment| normalized.contains(segment))
}

