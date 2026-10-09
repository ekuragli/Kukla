use std::fs::File;
use std::io::Read;
use std::path::Path;

/// DOCX (WordprocessingML) dosyasından düz metin çıkarır.
///
/// DOCX de bir ZIP arşividir; belge `word/document.xml` içinde tutulur.
pub fn read_docx_text(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("Dosya açılamadı: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|_| "Dosya bir DOCX (ZIP) arşivi değil.".to_string())?;

    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|_| "DOCX içinde word/document.xml bulunamadı.".to_string())?
        .read_to_string(&mut xml)
        .map_err(|e| format!("DOCX içeriği okunamadı: {}", e))?;

    Ok(xml_to_text(&xml))
}

fn xml_to_text(xml: &str) -> String {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut out = String::with_capacity(xml.len());
    let mut skip_depth: usize = 0;
    let mut pending_break = false;

    loop {
        let event = match reader.read_event() {
            Ok(event) => event,
            Err(_) => break,
        };
        let is_empty = matches!(event, quick_xml::events::Event::Empty(_));
        match event {
            quick_xml::events::Event::Start(tag) | quick_xml::events::Event::Empty(tag) => {
                let name = String::from_utf8_lossy(tag.name().as_ref()).to_ascii_lowercase();
                if skip_depth > 0 {
                    if !is_empty {
                        skip_depth += 1;
                    }
                    continue;
                }
                match name.as_str() {
                    "w:p" | "w:tr" | "w:br" | "w:cr" => {
                        if !out.is_empty() && !out.ends_with('\n') && !out.ends_with('\t') {
                            out.push('\n');
                        }
                        pending_break = false;
                    }
                    "w:tab" => out.push('\t'),
                    "w:tc" => out.push('\t'),
                    "w:sectPr" | "w:tblPr" | "w:trPr" | "w:pPr" => {
                        if !is_empty {
                            skip_depth = 1;
                        }
                    }
                    _ => {}
                }
            }
            quick_xml::events::Event::End(tag) => {
                let name = String::from_utf8_lossy(tag.name().as_ref()).to_ascii_lowercase();
                if skip_depth > 0 {
                    skip_depth -= 1;
                    continue;
                }
                match name.as_str() {
                    "w:p" | "w:tr" => pending_break = true,
                    "w:tc" => pending_break = true,
                    _ => {}
                }
            }
            quick_xml::events::Event::Text(text) => {
                if skip_depth > 0 {
                    continue;
                }
                let raw = reader
                    .decoder()
                    .decode(text.as_ref())
                    .map(|value| value.into_owned())
                    .unwrap_or_else(|_| String::from_utf8_lossy(text.as_ref()).into_owned());
                let unescaped = quick_xml::escape::unescape(&raw)
                    .map(|value| value.into_owned())
                    .unwrap_or_else(|_| raw.clone());
                if unescaped.trim().is_empty() {
                    continue;
                }
                if pending_break {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                    pending_break = false;
                }
                out.push_str(&unescaped);
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }

    out.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
