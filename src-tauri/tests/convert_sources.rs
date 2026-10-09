use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use kukla_lib::core::docx_read::read_docx_text;
use kukla_lib::core::udf::parse_udf;
use kukla_lib::utils::path_validation::validate_input_path;

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_file(suffix: &str, bytes: &[u8]) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "kukla_test_{}_{}.{}",
        std::process::id(),
        id,
        suffix
    ));
    fs::write(&path, bytes).expect("temp dosya yazılamadı");
    path
}

fn make_zip(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default();
        for (name, content) in entries {
            writer.start_file(*name, options).expect("arşiv girdisi");
            writer.write_all(content.as_bytes()).expect("arşiv içeriği");
        }
        writer.finish().expect("arşiv kapatılamadı");
    }
    cursor.into_inner()
}

#[test]
fn parses_udf_content_xml_to_text() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<document>
  <par>EGM. 12. Hukuk Dairesi</par>
  <par>Esas No: 2023/4567 &amp; Karar No: 2023/8901</par>
  <table>
    <tr><cell>Davacı</cell><cell>A. Yılmaz</cell></tr>
    <tr><cell>Davalı</cell><cell>B. Ltd. Şti.</cell></tr>
  </table>
  <par>Hüküm: Karar onanmıştır.</par>
</document>"#;

    let bytes = make_zip(&[
        ("content.xml", xml),
        ("documentproperties.xml", "<properties/>"),
    ]);
    let path = temp_file("udf", &bytes);
    let doc = parse_udf(&path).expect("UDF çözümlenmeli");
    let _ = fs::remove_file(&path);

    assert!(doc.text.contains("EGM. 12. Hukuk Dairesi"));
    assert!(doc.text.contains("Esas No: 2023/4567 & Karar No: 2023/8901"));
    assert!(doc.text.contains("Davacı"));
    assert!(doc.text.contains("Hüküm: Karar onanmıştır."));
    assert!(!doc.has_signature);
    assert!(doc.entries.iter().any(|name| name == "content.xml"));
}

#[test]
fn detects_udf_signature_entry() {
    let bytes = make_zip(&[("content.xml", "<doc><par>İmzalı belge</par></doc>"), ("sign.sgn", "sig")]);
    let path = temp_file("udf", &bytes);
    let doc = parse_udf(&path).expect("UDF çözümlenmeli");
    let _ = fs::remove_file(&path);

    assert!(doc.has_signature);
    assert!(doc.text.contains("İmzalı belge"));
}

#[test]
fn rejects_udf_without_content_xml() {
    let bytes = make_zip(&[("readme.txt", "bu bir udf değil")]);
    let path = temp_file("udf", &bytes);
    let err = parse_udf(&path).unwrap_err();
    let _ = fs::remove_file(&path);
    assert!(err.contains("content.xml"));
}

#[test]
fn rejects_non_zip_file_as_udf() {
    let path = temp_file("udf", b"bu metin dosyasi bir arsip degil");
    let err = parse_udf(&path).unwrap_err();
    let _ = fs::remove_file(&path);
    assert!(err.contains("ZIP"));
}

#[test]
fn parses_docx_document_xml() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>İlk paragraf</w:t></w:r></w:p>
    <w:p><w:r><w:t>Sütun A</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>Sütun B</w:t></w:r></w:p>
    <w:sectPr/>
  </w:body>
</w:document>"#;

    let bytes = make_zip(&[("word/document.xml", xml), ("[Content_Types].xml", "<Types/>")]);
    let path = temp_file("docx", &bytes);
    let text = read_docx_text(&path).expect("DOCX çözümlenmeli");
    let _ = fs::remove_file(&path);

    assert!(text.contains("İlk paragraf"));
    assert!(text.contains("Sütun A"));
    assert!(text.contains("Sütun B"));
    assert!(text.contains('\t'));
    assert!(text.lines().count() >= 2);
}

#[test]
fn rejects_docx_without_document_xml() {
    let bytes = make_zip(&[("word/styles.xml", "<styles/>")]);
    let path = temp_file("docx", &bytes);
    let err = read_docx_text(&path).unwrap_err();
    let _ = fs::remove_file(&path);
    assert!(err.contains("word/document.xml"));
}

#[test]
fn validate_input_path_accepts_converter_formats() {
    for suffix in ["txt", "udf", "docx"] {
        let path = temp_file(suffix, b"icerik");
        let result = validate_input_path(
            path.to_str().expect("utf8 yol"),
            &["pdf", "docx", "txt", "md", "udf", "zip"],
        );
        let _ = fs::remove_file(&path);
        assert!(result.is_ok(), ".{} reddedildi", suffix);
    }
}

#[test]
fn validate_input_path_rejects_unknown_extension() {
    let path = temp_file("exe", b"m");
    let err = validate_input_path(
        path.to_str().expect("utf8 yol"),
        &["pdf", "docx", "txt", "md", "udf", "zip"],
    )
    .unwrap_err();
    let _ = fs::remove_file(&path);
    assert!(err.contains("Desteklenmeyen"));
}
