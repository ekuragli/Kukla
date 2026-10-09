use serde::Deserialize;
use tauri::State;

use crate::models::LlmResponse;
use crate::AppState;

const MIN_SUMMARY_CONTEXT_CHARS: usize = 500;
const MAX_SUMMARY_CONTEXT_CHARS: usize = 6_000;

#[derive(Debug, Default, Deserialize)]
struct SummaryPayload {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    ratio_decidendi: String,
    #[serde(default)]
    source_citation: String,
    #[serde(default)]
    hallucination_score: f64,
}

#[tauri::command]
pub async fn summarize_decision(
    state: State<'_, AppState>,
    decision_id: i64,
    context: String,
    query: Option<String>,
    request_id: Option<String>,
) -> Result<LlmResponse, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    state.audit.log(
        crate::models::AuditEventType::Summarize,
        Some(&format!("decision_{}", decision_id)),
    );

    let trimmed = context.trim();
    if trimmed.chars().count() < MIN_SUMMARY_CONTEXT_CHARS {
        return Err(
            "Özet için yeterli karar metni yok. Tam metin yüklenene kadar bekleyin veya tekrar deneyin."
                .to_string(),
        );
    }

    let context = crate::utils::truncate_prompt_text(trimmed, MAX_SUMMARY_CONTEXT_CHARS);
    let summary_query = query
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty())
        .unwrap_or_else(|| "Bu kararı özetle".to_string());

    let cache_key = crate::utils::fnv1a_hex(&format!("{}\n{}", summary_query, context));
    if let Some(payload) = read_summary_cache(&state, decision_id, &cache_key) {
        tracing::info!(
            "Karar özeti önbellekten döndürüldü (karar {}, {} karakter)",
            decision_id,
            context.chars().count()
        );
        return Ok(payload);
    }

    let prompt = build_summary_prompt(&summary_query, &context);
    let started = std::time::Instant::now();
    let reply = crate::commands::opencode::run_prompt_streaming(
        &state,
        &prompt,
        request_id.as_deref(),
        crate::commands::opencode::PromptBudget::SUMMARY,
    )
    .await?;
    tracing::info!(
        "Karar özeti opencode üzerinden üretildi (model: {}, prompt: {} karakter, süre: {:.1}s, yanıt: {} karakter)",
        reply.model.as_deref().unwrap_or("bilinmiyor"),
        prompt.chars().count(),
        started.elapsed().as_secs_f64(),
        reply.text.chars().count()
    );

    let response = parse_summary_reply(&reply.text);
    write_summary_cache(&state, decision_id, &cache_key, &response);
    Ok(response)
}

fn read_summary_cache(
    state: &AppState,
    decision_id: i64,
    cache_key: &str,
) -> Option<LlmResponse> {
    let payload = state
        .db
        .read()
        .ok()?
        .get_llm_summary(decision_id, cache_key)
        .ok()??;
    serde_json::from_str::<LlmResponse>(&payload).ok()
}

fn write_summary_cache(
    state: &AppState,
    decision_id: i64,
    cache_key: &str,
    response: &LlmResponse,
) {
    let Ok(payload) = serde_json::to_string(response) else {
        return;
    };
    let result = state
        .db
        .read()
        .map_err(|_| anyhow::anyhow!("Veritabanı kilitli."))
        .and_then(|db| db.set_llm_summary(decision_id, cache_key, &payload));
    if let Err(error) = result {
        tracing::warn!("Karar özeti önbelleğe yazılamadı: {}", error);
    }
}

fn build_summary_prompt(query: &str, context: &str) -> String {
    format!(
        "Sen Türk hukuku ve Yargıtay içtihatları konusunda uzman bir asistansın.\n\
         Aşağıdaki karar metnini özetle ve kararın ratio decidendi (hukuki dayanağını) çıkar.\n\
         Odak sorusu: {query}\n\n\
         ZORUNLU KURALLAR:\n\
         - Yanıt dili TÜRKÇE olacak; summary, ratio_decidendi ve source_citation alanlarının\
         değerlerinde tek bir İngilizce kelime bile kullanma (JSON anahtar adları hariç).\n\
         - Dosya, terminal, tarayıcı veya başka bir araç KULLANMA; yalnızca yanıt metnini üret.\n\
         - Yanıtın SADECE tek bir JSON nesnesi olsun; JSON dışında hiçbir açıklama, kod bloğu veya\
         ek metin yazma.\n\
         - Şema:\n\
         {{\n\
           \"summary\": \"kararın 4-6 cümlelik özeti\",\n\
           \"ratio_decidendi\": \"kararın hukuki dayanağı, 2-3 cümle\",\n\
           \"source_citation\": \"esas/karar numarası, daire ve tarih metinde varsa; yoksa boş string\",\n\
           \"hallucination_score\": 0.0 ile 1.0 arası; özetin karar metnine dayanma riski (uydurma riski)\n\
         }}\n\n\
         KARAR METNİ:\n\
         {context}"
    )
}

fn parse_summary_reply(raw: &str) -> LlmResponse {
    let payload = extract_json(raw);
    let summary = if payload.summary.trim().is_empty() {
        tracing::warn!(
            "Özet yanıtında JSON bulunamadı, ham metin kullanılıyor ({} karakter)",
            raw.chars().count()
        );
        raw.trim().to_string()
    } else {
        payload.summary.trim().to_string()
    };

    LlmResponse {
        summary,
        ratio_decidendi: payload.ratio_decidendi.trim().to_string(),
        source_citation: payload.source_citation.trim().to_string(),
        hallucination_score: payload.hallucination_score.clamp(0.0, 1.0),
    }
}

fn extract_json(raw: &str) -> SummaryPayload {
    let start = raw.find('{');
    let end = raw.rfind('}');
    match (start, end) {
        (Some(start), Some(end)) if end >= start => {
            serde_json::from_str(&raw[start..=end]).unwrap_or_default()
        }
        _ => SummaryPayload::default(),
    }
}
