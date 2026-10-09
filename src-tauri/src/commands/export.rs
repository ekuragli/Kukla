use std::collections::HashMap;
use std::io::{BufWriter, Cursor};

use docx_rs::{Docx, Paragraph, Run};
use printpdf::{BuiltinFont, Mm, PdfDocument, PdfDocumentReference};
use tauri::State;

use crate::models::{ExportDecisionContext, ExportTarget};
use crate::AppState;

const FONT_SIZE: f32 = 9.0;
const LINE_HEIGHT_MM: f32 = 5.0;
const LEFT_MARGIN: f32 = 15.0;
const TOP_MARGIN: f32 = 280.0;
const BOTTOM_MARGIN: f32 = 20.0;
const MAX_CHARS_PER_LINE: usize = 88;

#[tauri::command]
pub async fn export_report(
    state: State<'_, AppState>,
    decision_ids: Vec<i64>,
    format: String,
    contexts: Option<Vec<ExportDecisionContext>>,
) -> Result<String, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    state.audit.log_export(&format);

    if decision_ids.is_empty() {
        return Err("Dışa aktarılacak karar seçilmedi.".to_string());
    }

    let format_lower = format.to_lowercase();
    let target = match format_lower.as_str() {
        "txt" => ExportTarget::Txt,
        "docx" => ExportTarget::Docx,
        "pdf" => ExportTarget::Pdf,
        _ => {
            return Err(format!(
                "Desteklenmeyen format: {}. txt, docx veya pdf kullanın.",
                format
            ));
        }
    };

    let report_content = generate_report_text(&state, &decision_ids, contexts.as_deref())
        .await
        .map_err(|e| format!("Rapor oluşturulamadı: {}", e))?;

    let ext = match target {
        ExportTarget::Txt => "txt",
        ExportTarget::Docx => "docx",
        ExportTarget::Pdf => "pdf",
    };

    let now = chrono::Utc::now();
    let file_name = format!(
        "kukla_rapor_{}_{}.{}",
        now.format("%Y%m%d_%H%M%S"),
        now.timestamp_subsec_millis(),
        ext
    );

    let desktop = dirs_next::desktop_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let output_path = desktop.join(&file_name);

    let file_bytes = match target {
        ExportTarget::Txt => report_content.into_bytes(),
        ExportTarget::Docx => build_docx_bytes(&report_content, Some("KUKLA İÇTİHAT RAPORU"))
            .map_err(|e| format!("DOCX oluşturulamadı: {}", e))?,
        ExportTarget::Pdf => build_pdf_bytes(&report_content)
            .map_err(|e| format!("PDF oluşturulamadı: {}", e))?,
    };

    std::fs::write(&output_path, file_bytes).map_err(|e| format!("Dosya yazılamadı: {}", e))?;

    Ok(output_path.to_string_lossy().to_string())
}

pub(crate) fn write_text_to_desktop(
    file_prefix: &str,
    content: &str,
    target: ExportTarget,
    docx_title: Option<&str>,
) -> Result<String, String> {
    let ext = match target {
        ExportTarget::Txt => "txt",
        ExportTarget::Docx => "docx",
        ExportTarget::Pdf => "pdf",
    };

    let now = chrono::Utc::now();
    let file_name = format!(
        "{}_{}_{}.{}",
        file_prefix,
        now.format("%Y%m%d_%H%M%S"),
        now.timestamp_subsec_millis(),
        ext
    );

    let desktop = dirs_next::desktop_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let output_path = desktop.join(&file_name);

    let file_bytes = match target {
        ExportTarget::Txt => content.to_string().into_bytes(),
        ExportTarget::Docx => build_docx_bytes(content, docx_title)
            .map_err(|e| format!("DOCX oluşturulamadı: {}", e))?,
        ExportTarget::Pdf => build_pdf_bytes(content)
            .map_err(|e| format!("PDF oluşturulamadı: {}", e))?,
    };

    std::fs::write(&output_path, file_bytes).map_err(|e| format!("Dosya yazılamadı: {}", e))?;

    Ok(output_path.to_string_lossy().to_string())
}

/// Bir bölümün (içtihat metni, özet, ratio decidendi, ...) tek başına dosyaya
/// yazılması. `kind` dosya adındaki ön ek için kullanılır (örn. `ozet`).
#[tauri::command]
pub fn export_section(
    state: State<'_, AppState>,
    kind: String,
    title: String,
    body: String,
    format: String,
) -> Result<String, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err("Dışa aktarılacak içerik boş.".to_string());
    }

    let target = parse_export_target(&format)?;
    let safe_kind: String = kind
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(32)
        .collect();
    let safe_kind = if safe_kind.is_empty() {
        "icerik".to_string()
    } else {
        safe_kind
    };

    let heading = title.trim();
    let content = if heading.is_empty() {
        trimmed.to_string()
    } else {
        format!("{}\n{}\n\n{}", heading, "=".repeat(heading.chars().count()), trimmed)
    };

    state
        .audit
        .log(crate::models::AuditEventType::Export, Some(&safe_kind));

    write_text_to_desktop(&format!("kukla_{}", safe_kind), &content, target, None)
}

pub(crate) fn parse_export_target(format: &str) -> Result<ExportTarget, String> {
    match format.to_lowercase().as_str() {
        "txt" => Ok(ExportTarget::Txt),
        "docx" => Ok(ExportTarget::Docx),
        "pdf" => Ok(ExportTarget::Pdf),
        other => Err(format!(
            "Desteklenmeyen format: {}. txt, docx veya pdf kullanın.",
            other
        )),
    }
}

pub(crate) fn build_docx_bytes(
    report_text: &str,
    title: Option<&str>,
) -> Result<Vec<u8>, anyhow::Error> {
    let mut docx = Docx::new();
    if let Some(title) = title.map(str::trim).filter(|value| !value.is_empty()) {
        docx = docx.add_paragraph(
            Paragraph::new().add_run(Run::new().add_text(title).bold().size(28)),
        );
    }

    for line in report_text.lines() {
        docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text(line)));
    }

    let mut buffer = Cursor::new(Vec::new());
    docx.build().pack(&mut buffer)?;
    Ok(buffer.into_inner())
}

fn load_pdf_font(doc: &PdfDocumentReference) -> Result<printpdf::IndirectFontRef, anyhow::Error> {
    #[cfg(windows)]
    {
        for path in [
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\segoeui.ttf",
        ] {
            if let Ok(bytes) = std::fs::read(path) {
                if let Ok(font) = doc.add_external_font(Cursor::new(bytes)) {
                    return Ok(font);
                }
            }
        }
    }

    doc.add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| anyhow::anyhow!("PDF fontu yüklenemedi: {}", e))
}

pub fn char_count(text: &str) -> usize {
    text.chars().count()
}

pub fn wrap_text_line(line: &str, max_chars: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }
    if char_count(line) <= max_chars {
        return vec![line.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();

    for word in line.split_whitespace() {
        if char_count(word) > max_chars {
            if !current.is_empty() {
                lines.push(current);
                current = String::new();
            }
            let mut chunk = String::new();
            for ch in word.chars() {
                chunk.push(ch);
                if char_count(&chunk) >= max_chars {
                    lines.push(chunk);
                    chunk = String::new();
                }
            }
            if !chunk.is_empty() {
                current = chunk;
            }
            continue;
        }

        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };

        if char_count(&candidate) > max_chars && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            current = candidate;
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        vec![line.to_string()]
    } else {
        lines
    }
}

fn wrap_report_for_pdf(report_text: &str) -> Vec<String> {
    report_text
        .lines()
        .flat_map(|line| wrap_text_line(line, MAX_CHARS_PER_LINE))
        .collect()
}

pub(crate) fn build_pdf_bytes(report_text: &str) -> Result<Vec<u8>, anyhow::Error> {
    let (doc, first_page, first_layer) =
        PdfDocument::new("Kukla İçtihat Raporu", Mm(210.0), Mm(297.0), "Layer 1");
    let font = load_pdf_font(&doc)?;
    let lines = wrap_report_for_pdf(report_text);
    let max_lines_per_page =
        ((TOP_MARGIN - BOTTOM_MARGIN) / LINE_HEIGHT_MM).floor() as usize;

    for (page_idx, chunk) in lines.chunks(max_lines_per_page.max(1)).enumerate() {
        let (page, layer_id) = if page_idx == 0 {
            (first_page, first_layer)
        } else {
            doc.add_page(Mm(210.0), Mm(297.0), "Layer 1")
        };

        let layer = doc.get_page(page).get_layer(layer_id);
        let mut y = TOP_MARGIN;

        for line in chunk {
            layer.use_text(line.clone(), FONT_SIZE, Mm(LEFT_MARGIN), Mm(y), &font);
            y -= LINE_HEIGHT_MM;
            if y < BOTTOM_MARGIN {
                break;
            }
        }
    }

    let mut buffer = BufWriter::new(Vec::new());
    doc.save(&mut buffer)?;
    Ok(buffer.into_inner()?)
}

async fn fetch_online_text_with_fallback(
    state: &AppState,
    document_id: i64,
    source_type: Option<&str>,
) -> Option<String> {
    let doc_id = document_id.to_string();

    match source_type {
        Some("yargitay") => {
            if let Ok(doc) = state.yargitay_client.get_document(&doc_id).await {
                if !doc.text.trim().is_empty() {
                    return Some(doc.text);
                }
            }
        }
        Some("bedesten") => {
            if let Ok(doc) = state.bedesten_client.get_document_markdown(&doc_id).await {
                if !doc.markdownContent.trim().is_empty() {
                    return Some(doc.markdownContent);
                }
            }
        }
        _ => {}
    }

    if let Ok(doc) = state.yargitay_client.get_document(&doc_id).await {
        if !doc.text.trim().is_empty() {
            return Some(doc.text);
        }
    }

    if let Ok(doc) = state.bedesten_client.get_document_markdown(&doc_id).await {
        if !doc.markdownContent.trim().is_empty() {
            return Some(doc.markdownContent);
        }
    }

    None
}

fn collect_indexed_chunks(
    vector_db: &crate::db::vector_store::VectorDb,
) -> Vec<crate::models::ChunkMetadata> {
    let mut chunks = Vec::new();
    for source in ["user_uploaded", "bedesten_online"] {
        if let Ok(mut found) = vector_db.list_chunks(source) {
            chunks.append(&mut found);
        }
    }
    chunks
}

fn chunks_for_decision(
    chunks: &[crate::models::ChunkMetadata],
    decision_id: i64,
) -> Vec<&crate::models::ChunkMetadata> {
    let doc_id = decision_id.to_string();
    chunks
        .iter()
        .filter(|c| {
            crate::utils::chunk_meta::decision_id_from_chunk(c) == Some(decision_id)
                || crate::utils::chunk_meta::document_id_from_chunk(c).as_deref() == Some(&doc_id)
        })
        .collect()
}

struct PreparedDecisionExport {
    context: Option<ExportDecisionContext>,
    local_decision: Option<crate::models::Decision>,
    chunk_texts: Vec<String>,
    needs_online_fetch: bool,
}

async fn generate_report_text(
    state: &AppState,
    decision_ids: &[i64],
    contexts: Option<&[ExportDecisionContext]>,
) -> Result<String, anyhow::Error> {
    use std::io::Write;

    let context_map: HashMap<i64, ExportDecisionContext> = contexts
        .unwrap_or(&[])
        .iter()
        .cloned()
        .map(|ctx| (ctx.decision_id, ctx))
        .collect();

    let (local_decisions, chunks) = {
        let db = state
            .db
            .read()
            .map_err(|_| anyhow::anyhow!("Veritabanı kilitli."))?;
        let vector_db = state
            .vector_db
            .read()
            .map_err(|_| anyhow::anyhow!("Vector store kilitli."))?;

        let mut local_decisions = HashMap::new();
        for id in decision_ids {
            local_decisions.insert(*id, db.get_decision(*id)?);
        }

        let chunks = collect_indexed_chunks(&vector_db);
        (local_decisions, chunks)
    };

    let mut prepared = Vec::new();
    for id in decision_ids {
        let ctx = context_map.get(id).cloned();
        let has_context_text = ctx
            .as_ref()
            .and_then(|c| c.full_text.as_ref())
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false);

        let matching_chunks = chunks_for_decision(&chunks, *id);
        let chunk_texts: Vec<String> = matching_chunks
            .iter()
            .map(|c| c.text_chunk.clone())
            .collect();

        let needs_online_fetch =
            !has_context_text && chunk_texts.is_empty();

        prepared.push(PreparedDecisionExport {
            context: ctx,
            local_decision: local_decisions.get(id).cloned().flatten(),
            chunk_texts,
            needs_online_fetch,
        });
    }

    let mut online_texts = HashMap::new();
    for (id, item) in decision_ids.iter().zip(prepared.iter()) {
        if !item.needs_online_fetch {
            continue;
        }
        let source_type = item.context.as_ref().and_then(|c| c.source_type.clone());
        if let Some(text) =
            fetch_online_text_with_fallback(state, *id, source_type.as_deref()).await
        {
            online_texts.insert(*id, text);
        }
    }

    let mut report = Vec::new();
    writeln!(report, "========================================")?;
    writeln!(report, "KUKLA İÇTİHAT RAPORU")?;
    writeln!(report, "Oluşturma Tarihi: {}", chrono::Utc::now().format("%d.%m.%Y %H:%M"))?;
    writeln!(report, "========================================\n")?;

    for (id, item) in decision_ids.iter().zip(prepared.iter()) {
        let ctx = item.context.as_ref();
        writeln!(report, "--- Karar ID: {} ---", id)?;

        let mut wrote_metadata = false;

        if let Some(ctx) = ctx {
            if let Some(ref source_type) = ctx.source_type {
                let label = match source_type.as_str() {
                    "yargitay" => "Yargıtay",
                    "bedesten" => "Bedesten",
                    _ => source_type.as_str(),
                };
                writeln!(report, "Kaynak: {}", label)?;
                wrote_metadata = true;
            }
            if let Some(ref esas) = ctx.esas_no {
                writeln!(report, "Esas No: {}", esas)?;
                wrote_metadata = true;
            }
            if let Some(ref karar) = ctx.karar_no {
                writeln!(report, "Karar No: {}", karar)?;
                wrote_metadata = true;
            }
            if let Some(ref daire) = ctx.daire {
                writeln!(report, "Daire: {}", daire)?;
                wrote_metadata = true;
            }
            if let Some(ref summary) = ctx.summary {
                if !summary.trim().is_empty() {
                    writeln!(report)?;
                    writeln!(report, "YZ Özeti:")?;
                    writeln!(report, "{}", summary)?;
                }
            }
            if let Some(ref ratio) = ctx.ratio_decidendi {
                if !ratio.trim().is_empty() {
                    writeln!(report)?;
                    writeln!(report, "Ratio Decidendi:")?;
                    writeln!(report, "{}", ratio)?;
                }
            }
            if let Some(ref citation) = ctx.source_citation {
                if !citation.trim().is_empty() {
                    writeln!(report)?;
                    writeln!(report, "Kaynak Referansı:")?;
                    writeln!(report, "{}", citation)?;
                }
            }
        }

        if let Some(ref decision) = item.local_decision {
            if !wrote_metadata {
                writeln!(report, "Kaynak: {}", decision.source.as_str())?;
            }
            if ctx.is_none() || ctx.and_then(|c| c.esas_no.as_ref()).is_none() {
                if let Some(ref esas) = decision.esas_no {
                    writeln!(report, "Esas No: {}", esas)?;
                }
            }
            if ctx.is_none() || ctx.and_then(|c| c.karar_no.as_ref()).is_none() {
                if let Some(ref karar) = decision.karar_no {
                    writeln!(report, "Karar No: {}", karar)?;
                }
            }
            if ctx.is_none() || ctx.and_then(|c| c.daire.as_ref()).is_none() {
                if let Some(ref daire) = decision.daire {
                    writeln!(report, "Daire: {}", daire)?;
                }
            }
            if ctx.is_none() {
                if let Some(ref summary) = decision.summary {
                    writeln!(report)?;
                    writeln!(report, "Özet:")?;
                    writeln!(report, "{}", summary)?;
                }
            }
        }

        let mut body_written = false;

        if let Some(ctx) = ctx {
            if let Some(ref full_text) = ctx.full_text {
                if !full_text.trim().is_empty() {
                    writeln!(report)?;
                    writeln!(report, "Karar Metni:")?;
                    writeln!(report, "{}", full_text)?;
                    body_written = true;
                }
            }
        }

        if !body_written && !item.chunk_texts.is_empty() {
            writeln!(report)?;
            writeln!(report, "Karar Metni:")?;
            for chunk_text in &item.chunk_texts {
                writeln!(report, "{}", chunk_text)?;
            }
            body_written = true;
        }

        if !body_written {
            if let Some(text) = online_texts.get(id) {
                writeln!(report)?;
                writeln!(report, "Karar Metni:")?;
                writeln!(report, "{}", text)?;
                body_written = true;
            }
        }

        if !body_written {
            writeln!(report, "(Karar metni bulunamadı — detay ekranında metin yüklenene kadar bekleyin)")?;
        }

        writeln!(report)?;
    }

    writeln!(report, "\n========================================")?;
    writeln!(report, "Rapor Sonu")?;
    writeln!(report, "Bu rapor Kukla tarafından oluşturulmuştur.")?;
    writeln!(report, "Tavsiye niteliğindedir, hukuki bağlayıcılığı yoktur.")?;
    writeln!(report, "Kaynak: Yargıtay Karar Arama, Adalet Bakanlığı Bedesten API ve yerel veritabanı")?;

    String::from_utf8(report)
        .map_err(|e| anyhow::anyhow!("Rapor format hatası: {}", e))
}