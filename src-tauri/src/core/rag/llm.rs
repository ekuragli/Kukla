use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::OnceLock;

use crate::core::rag::petition_templates;
use crate::models::{LlmResponse, PetitionDraftResponse, PetitionType};

pub fn build_petition_prompt(
    petition_type: PetitionType,
    case_facts: &str,
    decision_context: &str,
    citation: &str,
    ratio_decidendi: Option<&str>,
    ai_summary: Option<&str>,
) -> String {
    let template_label = petition_type.label();
    let structure = petition_templates::structure_instructions(petition_type);
    let ratio_block = ratio_decidendi
        .filter(|s| !s.trim().is_empty())
        .map(|r| format!("\n\nRatio Decidendi (Hükme Esas Gerekçe):\n{}", r))
        .unwrap_or_default();
    let summary_block = ai_summary
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("\n\nKarar Özeti:\n{}", s))
        .unwrap_or_default();

    format!(
        r#"Sen deneyimli bir Türk hukuk asistanısın. Avukatın müvekkili adına hazırlayacağı dilekçe için TASLAK metin üret.
Sadece sağlanan dava özeti ve içtihat kararına dayan. Uydurma olay, tarih veya karar referansı ekleme.

Dilekçe Türü: {}
{}

Müvekkil / Dava Özeti:
{}

İçtihat Kararı Referansı:
{}

İçtihat Karar Metni (kaynak):
{}{}

Yanıtını AYNEN şu bölüm ayraçlarıyla ver (ayraç satırlarını değiştirme):

---DILEKCE---
(Seçilen dilekçe türüne uygun, Türk hukuk usulüne göre yapılandırılmış taslak; taraf bilgileri için [DOLDURULACAK] yer tutucuları kullan)

---ICTIHAT_REFERANSLARI---
(Kullanılan içtihat kararının tam referansı ve kısa gerekçe özeti)

---UYARI---
(Bu metnin YZ destekli {} taslağı olduğu, avukat tarafından gözden geçirilmesi gerektiği uyarısı)

Sadece sağlanan bilgilere dayan."#,
        template_label,
        structure,
        case_facts,
        citation,
        decision_context,
        ratio_block + &summary_block,
        template_label
    )
}

pub fn build_summarize_prompt(context: &str, query: &str) -> String {
    format!(
        r#"Sen bir hukuk asistanısın. Sadece sağlanan karar metnine dayalı cevap ver. Kaynak dışı bilgi üretme.

Kullanıcının Sorusu: {}
Karar Metni:
{}

Yanıtını AYNEN şu bölüm ayraçlarıyla ver (ayraç satırlarını değiştirme):

---KISA_OZET---
(max 300 kelime özet)

---RATIO_DECIDENDI---
(hükme esas teşkil eden gerekçe)

---KAYNAK---
(esas no, karar no, tarih, daire — metinde varsa)

Sadece yukarıdaki karar metninde yazan bilgileri kullan."#,
        query, context
    )
}

pub fn check_source_overlap(response: &str, source: &str) -> f64 {
    crate::core::rag::hybrid::keyword_overlap_score(response, source).clamp(0.0, 1.0)
}

pub struct ParsedLlmSections {
    pub summary: String,
    pub ratio_decidendi: String,
    pub source_citation: String,
}

pub struct ParsedPetitionSections {
    pub draft: String,
    pub cited_decisions: String,
    pub disclaimer: String,
}

pub fn parse_petition_llm_response(raw: &str) -> ParsedPetitionSections {
    let draft = extract_section(raw, "---DILEKCE---", "---ICTIHAT_REFERANSLARI---")
        .or_else(|| extract_section(raw, "DİLEKÇE", "İÇTİHAT"))
        .unwrap_or_else(|| raw.trim().to_string());

    let cited_decisions = extract_section(raw, "---ICTIHAT_REFERANSLARI---", "---UYARI---")
        .or_else(|| extract_section(raw, "İÇTİHAT REFERANSLARI", "UYARI"))
        .unwrap_or_default();

    let disclaimer = extract_section(raw, "---UYARI---", "")
        .or_else(|| extract_section(raw, "UYARI", ""))
        .unwrap_or_else(|| {
            "Bu metin YZ destekli bir taslaktır. Kullanılmadan önce avukat tarafından gözden geçirilmelidir."
                .to_string()
        });

    ParsedPetitionSections {
        draft,
        cited_decisions,
        disclaimer,
    }
}

pub fn parse_structured_llm_response(raw: &str) -> ParsedLlmSections {
    let summary = extract_section(raw, "---KISA_OZET---", "---RATIO_DECIDENDI---")
        .or_else(|| extract_section(raw, "KISA ÖZET", "RATIO DECIDENDI"))
        .unwrap_or_else(|| raw.trim().to_string());

    let ratio_decidendi = extract_section(raw, "---RATIO_DECIDENDI---", "---KAYNAK---")
        .or_else(|| extract_section(raw, "RATIO DECIDENDI", "KAYNAK"))
        .unwrap_or_default();

    let source_citation = extract_section(raw, "---KAYNAK---", "")
        .or_else(|| extract_section(raw, "KAYNAK REFERANSI", ""))
        .unwrap_or_default();

    ParsedLlmSections {
        summary,
        ratio_decidendi,
        source_citation,
    }
}

fn extract_section(text: &str, start_marker: &str, end_marker: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let start_lower = start_marker.to_lowercase();
    let start = lower.find(&start_lower)?;
    let content_start = start + start_marker.len();

    let content = if end_marker.is_empty() {
        text[content_start..].to_string()
    } else {
        let end_lower = end_marker.to_lowercase();
        let rest = &lower[content_start..];
        if let Some(end_offset) = rest.find(&end_lower) {
            text[content_start..content_start + end_offset].to_string()
        } else {
            text[content_start..].to_string()
        }
    };

    let trimmed = content.trim().trim_start_matches([':', '-', '\n', '\r']).trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}