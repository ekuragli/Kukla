use tauri::State;

use crate::core::rag::petition_templates;
use crate::models::{PetitionDraftResponse, PetitionTemplateInfo};
use crate::AppState;

#[tauri::command]
pub fn get_petition_templates() -> Vec<PetitionTemplateInfo> {
    petition_templates::all_templates()
}

const MIN_CASE_FACTS_CHARS: usize = 30;
const MIN_DECISION_CONTEXT_CHARS: usize = 200;
const MAX_CASE_FACTS_CHARS: usize = 5_000;
const MAX_DECISION_CONTEXT_CHARS: usize = 6_000;
/// Karar özeti (YZ) zaten dilekçe prompt'una ekleniyorsa karar metnini daha agresif kırp.
const MAX_DECISION_CONTEXT_WITH_SUMMARY_CHARS: usize = 4_000;

#[tauri::command]
pub async fn generate_petition_draft(
    state: State<'_, AppState>,
    decision_id: i64,
    case_facts: String,
    decision_context: String,
    citation: String,
    ratio_decidendi: Option<String>,
    ai_summary: Option<String>,
    petition_type: Option<String>,
    request_id: Option<String>,
) -> Result<PetitionDraftResponse, String> {
    crate::commands::auth_guard::require_auth(&state)?;

    let petition_type = petition_templates::parse_petition_type(
        petition_type.as_deref().unwrap_or("genel"),
    )?;

    state.audit.log(
        crate::models::AuditEventType::PetitionDraft,
        Some(&format!(
            "decision_{}_{}",
            decision_id,
            petition_type.as_str()
        )),
    );

    let trimmed_facts = case_facts.trim();
    if trimmed_facts.chars().count() < MIN_CASE_FACTS_CHARS {
        return Err(
            "Dilekçe taslağı için dava özeti çok kısa. En az birkaç cümle ile olayı özetleyin."
                .to_string(),
        );
    }

    let trimmed_context = decision_context.trim();
    if trimmed_context.chars().count() < MIN_DECISION_CONTEXT_CHARS {
        return Err(
            "Dilekçe taslağı için yeterli karar metni yok. Tam metin yüklenene kadar bekleyin."
                .to_string(),
        );
    }

    let trimmed_citation = citation.trim();
    if trimmed_citation.is_empty() {
        return Err("İçtihat referansı bulunamadı.".to_string());
    }

    let case_facts = crate::utils::truncate_prompt_text(trimmed_facts, MAX_CASE_FACTS_CHARS);
    let has_summary = ai_summary
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let context_cap = if has_summary {
        MAX_DECISION_CONTEXT_WITH_SUMMARY_CHARS
    } else {
        MAX_DECISION_CONTEXT_CHARS
    };
    let decision_context = crate::utils::truncate_prompt_text(trimmed_context, context_cap);

    let prompt = build_petition_prompt(
        &petition_type,
        &case_facts,
        &decision_context,
        trimmed_citation,
        ratio_decidendi.as_deref(),
        ai_summary.as_deref(),
    );

    let started = std::time::Instant::now();
    let reply = crate::commands::opencode::run_prompt_streaming(
        &state,
        &prompt,
        request_id.as_deref(),
        crate::commands::opencode::PromptBudget::PETITION,
    )
    .await?;
    tracing::info!(
        "Dilekçe taslağı opencode üzerinden üretildi (model: {}, prompt: {} karakter, süre: {:.1}s, yanıt: {} karakter)",
        reply.model.as_deref().unwrap_or("bilinmiyor"),
        prompt.chars().count(),
        started.elapsed().as_secs_f64(),
        reply.text.chars().count()
    );

    let payload = extract_draft_json(&reply.text);
    let draft = if payload.draft.trim().is_empty() {
        tracing::warn!(
            "Dilekçe yanıtında JSON bulunamadı, ham metin kullanılıyor ({} karakter)",
            reply.text.chars().count()
        );
        strip_code_fence(reply.text.trim())
    } else {
        payload.draft.trim().to_string()
    };

    if draft.trim().is_empty() {
        return Err("Model boş dilekçe taslağı döndürdü. Tekrar deneyin.".to_string());
    }

    Ok(PetitionDraftResponse {
        draft,
        cited_decisions: String::new(),
        disclaimer: "Bu metin YZ destekli bir taslaktır. Kullanılmadan önce avukat tarafından gözden geçirilmelidir.".to_string(),
        hallucination_score: payload.hallucination_score.clamp(0.0, 1.0),
        template_type: petition_type.as_str().to_string(),
        template_label: petition_type.label().to_string(),
    })
}

fn build_petition_prompt(
    petition_type: &crate::models::PetitionType,
    case_facts: &str,
    decision_context: &str,
    citation: &str,
    ratio_decidendi: Option<&str>,
    ai_summary: Option<&str>,
) -> String {
    let mut prompt = format!(
        "Sen Türk hukuku alanında deneyimli bir avukatsın. Aşağıdaki bilgilere dayanarak {} türünde\
         bir dilekçe taslağı hazırla.\n\n\
         ZORUNLU KURALLAR:\n\
         - Yanıt dili TÜRKÇE olacak; dilekçenin tamamında tek bir İngilizce kelime bile kullanma\
         (JSON anahtar adları hariç).\n\
         - Dosya, terminal, tarayıcı veya başka bir araç KULLANMA; yalnızca yanıt metnini üret.\n\
         - Yanıtın SADECE tek bir JSON nesnesi olsun; JSON dışında hiçbir açıklama veya kod bloğu\
         yazma.\n\
         - Şema: {{ \"draft\": \"dilekçenin tam metni\", \"hallucination_score\": 0.0 }}\n\
         - \"draft\" içinde uydurma İçtihat, Esas No, Karar No veya tarih ÜRETME; metinde verilen\
         bilgileri kullan.\n\
         - Dilekçe; başlık, taraflar, talep, olaylar, hukuki nedenler ve sonuç bölümlerini içersin,\
         sonuç kısmında dayanak olarak verilen içtihata atıf yapılsın.\n\n\
         DAVA ÖZETİ (olaylar):\n{}\n\n\
         KARAR METNİ (dayanak):\n{}\n\n\
         İÇTİHAT KÜNYESİ:\n{}\n",
        petition_type.label(),
        case_facts,
        decision_context,
        citation
    );

    if let Some(ratio) = ratio_decidendi.filter(|r| !r.trim().is_empty()) {
        prompt.push_str("\nRATIO DECIDENDI:\n");
        prompt.push_str(ratio.trim());
        prompt.push('\n');
    }
    if let Some(summary) = ai_summary.filter(|s| !s.trim().is_empty()) {
        prompt.push_str("\nKARAR ÖZETİ:\n");
        prompt.push_str(summary.trim());
        prompt.push('\n');
    }

    prompt
}

#[derive(Debug, Default, serde::Deserialize)]
struct DraftPayload {
    #[serde(default)]
    draft: String,
    #[serde(default)]
    hallucination_score: f64,
}

fn extract_draft_json(raw: &str) -> DraftPayload {
    let start = raw.find('{');
    let end = raw.rfind('}');
    match (start, end) {
        (Some(start), Some(end)) if end >= start => {
            serde_json::from_str(&raw[start..=end]).unwrap_or_default()
        }
        _ => DraftPayload::default(),
    }
}

fn strip_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    let without_fence = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    without_fence
        .strip_suffix("```")
        .unwrap_or(without_fence)
        .trim()
        .to_string()
}

#[tauri::command]
pub fn export_petition_draft(
    draft: String,
    cited_decisions: Option<String>,
    disclaimer: Option<String>,
    citation: String,
    format: String,
    template_label: Option<String>,
) -> Result<String, String> {
    let trimmed = draft.trim();
    if trimmed.is_empty() {
        return Err("Dışa aktarılacak dilekçe taslağı boş.".to_string());
    }

    let target = crate::commands::export::parse_export_target(&format)?;

    let title = template_label
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|label| format!("KUKLA DİLEKÇE TASLAĞI — {}", label.trim()))
        .unwrap_or_else(|| "KUKLA DİLEKÇE TASLAĞI".to_string());

    let mut body = title;
    body.push('\n');
    body.push_str(&"=".repeat(40));
    body.push('\n');
    body.push_str(trimmed);
    body.push_str("\n\n");

    if let Some(cited) = cited_decisions.as_deref().filter(|s| !s.trim().is_empty()) {
        body.push_str("İÇTİHAT REFERANSLARI\n");
        body.push_str(&"-".repeat(40));
        body.push('\n');
        body.push_str(cited.trim());
        body.push_str("\n\n");
    }

    body.push_str("KAYNAK KARAR\n");
    body.push_str(&"-".repeat(40));
    body.push('\n');
    body.push_str(citation.trim());
    body.push_str("\n\n");

    if let Some(note) = disclaimer.as_deref().filter(|s| !s.trim().is_empty()) {
        body.push_str("UYARI\n");
        body.push_str(&"-".repeat(40));
        body.push('\n');
        body.push_str(note.trim());
        body.push('\n');
    }

    crate::commands::export::write_text_to_desktop("kukla_dilekce", &body, target, None)
}
