#![allow(dead_code)]

pub mod path_validation;
pub mod chunk_meta;

use chrono::NaiveDate;

pub fn format_date(date: NaiveDate) -> String {
    date.format("%d.%m.%Y").to_string()
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%d.%m.%Y").ok()
}

pub fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max_chars).collect::<String>())
    }
}

/// LLM prompt'ları için kırpma: uzun metnin başı korunur, sonuna kısa bir kuyruk
/// eklenir. Ratio decidendi ve sonuç bölümü çoğunlukla metnin sonunda geçtiği için
/// yalnızca baştan kırpmak bu bilgiyi kaybettirir.
pub fn truncate_prompt_text(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let tail = (max_chars / 5).min(800);
    let head = max_chars.saturating_sub(tail);
    let head_text: String = text.chars().take(head).collect();
    let tail_text: String = text.chars().skip(count - tail).collect();
    format!("{}...\n[...]\n{}", head_text, tail_text.trim_start())
}

/// Önbellek anahtarı için sabit (çalıştırmalar arası aynı) 64-bit FNV-1a özeti.
pub fn fnv1a_hex(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:016x}", hash)
}

pub fn sanitize_query(query: &str) -> String {
    query.trim().to_string()
}

pub fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
