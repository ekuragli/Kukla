use std::fs::File;
use std::io::Read;
use std::path::Path;

/// UYAP Doküman Formatı (`.udf`) belgesi.
///
/// Teknik olarak bir ZIP arşividir; içinde `content.xml` (UYAP'a özel XML),
/// isteğe bağlı `documentproperties.xml` ve e-imzalı belgelerde `sign.sgn` bulunur.
#[derive(Debug, Default, Clone)]
pub struct UdfDocument {
    /// Belgenin düz metin hâli (paragraf ve tablo satırları korunur).
    pub text: String,
    /// Dosyada PKCS#7 e-imza (`sign.sgn`) var mı?
    pub has_signature: bool,
    /// Bulunan arşiv girdileri (hata ayıklama/uyarı için).
    pub entries: Vec<String>,
}

const BLOCK_TAGS: &[&str] = &[
    "par", "p", "para", "li", "item", "row", "tr", "cell", "td", "th", "h1", "h2", "h3", "h4",
    "h5", "h6", "table", "section", "blockquote", "pre",
];

const SKIP_TAGS: &[&str] = &["style", "styles", "metadata", "signature"];

/// `.udf` (veya `.udf.zip`) dosyasını okuyup düz metne çevirir.
pub fn parse_udf(path: &Path) -> Result<UdfDocument, String> {
    let file = File::open(path).map_err(|e| format!("Dosya açılamadı: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|_| "Dosya bir UDF (ZIP) arşivi değil.".to_string())?;

    let mut doc = UdfDocument::default();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| format!("Arşiv okunamadı: {}", e))?;
        doc.entries.push(entry.name().to_string());
    }

    let content_name = doc
        .entries
        .iter()
        .find(|name| name.eq_ignore_ascii_case("content.xml"))
        .cloned();
    let Some(content_name) = content_name else {
        return Err(
            "UDF içinde content.xml bulunamadı; dosya bozuk veya UDF değil olabilir.".to_string(),
        );
    };

    let mut xml = String::new();
    archive
        .by_name(&content_name)
        .map_err(|e| format!("content.xml okunamadı: {}", e))?
        .read_to_string(&mut xml)
        .map_err(|e| format!("content.xml çözümlenemedi: {}", e))?;

    doc.has_signature = doc
        .entries
        .iter()
        .any(|name| name.eq_ignore_ascii_case("sign.sgn"));
    doc.text = xml_to_text(&xml);
    Ok(doc)
}

/// UYAP XML'ini düz metne çevirir. Şema kapalı olduğu için genel kural kullanılır:
/// blok elemanları paragrafa, `<br>`/`<tab>` satır sonu/taba çevrilir.
fn xml_to_text(xml: &str) -> String {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut out = String::with_capacity(xml.len());
    let mut skip_depth: usize = 0;
    let mut pending_break = false;

    let push_break = |out: &mut String, pending: &mut bool| {
        if *pending || out.ends_with('\n') || out.is_empty() {
            *pending = false;
            return;
        }
        out.push('\n');
        *pending = false;
    };

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
                if SKIP_TAGS.contains(&name.as_str()) && !is_empty {
                    skip_depth = 1;
                    continue;
                }
                if name == "br" || name == "tab" || name == "linebreak" {
                    push_break(&mut out, &mut pending_break);
                    if name == "tab" {
                        out.push('\t');
                    }
                    continue;
                }
                if BLOCK_TAGS.contains(&name.as_str()) {
                    push_break(&mut out, &mut pending_break);
                    if !is_empty {
                        // kapanışta da satır sonu ekle
                        pending_break = true;
                    }
                }
            }
            quick_xml::events::Event::End(tag) => {
                let name = String::from_utf8_lossy(tag.name().as_ref()).to_ascii_lowercase();
                if skip_depth > 0 {
                    skip_depth -= 1;
                    continue;
                }
                if BLOCK_TAGS.contains(&name.as_str()) {
                    pending_break = true;
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
                    push_break(&mut out, &mut pending_break);
                }
                out.push_str(&unescaped);
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }

    normalize_lines(&out)
}

fn normalize_lines(text: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            if !lines.last().is_some_and(|last| last.trim().is_empty()) {
                lines.push("");
            }
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|last| last.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}
