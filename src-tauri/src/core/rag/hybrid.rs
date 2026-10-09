use std::collections::HashSet;

use super::vector_search::SearchHit;

/// Vektör benzerliği + anahtar kelime eşleşmesi ağırlıkları
pub const VECTOR_WEIGHT: f64 = 0.65;
pub const KEYWORD_WEIGHT: f64 = 0.35;

/// Sorguyu anlamlı token'lara böler (Türkçe karakterler korunur).
pub fn tokenize(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(|t| t.to_string())
        .collect()
}

/// Sorgu token'larının metin içindeki kapsama oranı (0.0–1.0).
pub fn keyword_overlap_score(query: &str, document: &str) -> f64 {
    let query_tokens: HashSet<String> = tokenize(query)
        .into_iter()
        .filter(|t| t.len() >= 3)
        .collect();

    if query_tokens.is_empty() {
        let short: HashSet<String> = tokenize(query).into_iter().collect();
        if short.is_empty() {
            return 0.0;
        }
        let doc_tokens: HashSet<String> = tokenize(document).into_iter().collect();
        let matches = short.intersection(&doc_tokens).count();
        return matches as f64 / short.len() as f64;
    }

    let doc_tokens: HashSet<String> = tokenize(document).into_iter().collect();
    let matches = query_tokens.intersection(&doc_tokens).count();
    matches as f64 / query_tokens.len() as f64
}

pub fn keyword_score_for_fields(
    query: &str,
    esas_no: Option<&str>,
    karar_no: Option<&str>,
    daire: Option<&str>,
    body: &str,
) -> f64 {
    let mut corpus = String::new();
    if let Some(v) = esas_no {
        corpus.push_str(v);
        corpus.push(' ');
    }
    if let Some(v) = karar_no {
        corpus.push_str(v);
        corpus.push(' ');
    }
    if let Some(v) = daire {
        corpus.push_str(v);
        corpus.push(' ');
    }
    corpus.push_str(body);
    keyword_overlap_score(query, &corpus)
}

pub fn hybrid_score(vector_score: f64, keyword_score: f64) -> f64 {
    (VECTOR_WEIGHT * vector_score + KEYWORD_WEIGHT * keyword_score).clamp(0.0, 1.0)
}

/// Vektör sonuçlarını anahtar kelime skoruyla yeniden sıralar.
pub fn rerank_hits(mut hits: Vec<SearchHit>, query: &str) -> Vec<SearchHit> {
    for hit in &mut hits {
        let keyword = keyword_overlap_score(query, &hit.metadata.text_chunk);
        hit.score = hybrid_score(hit.score, keyword);
    }
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits
}

pub fn score_to_percent(score: f64) -> u8 {
    (score.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// Bedesten API sonuçları zaten alaka sırasına göre gelir; sırayı skora çevirir.
pub fn bedesten_rank_score(index: usize) -> f64 {
    (1.0 - index as f64 * 0.03).clamp(0.55, 1.0)
}