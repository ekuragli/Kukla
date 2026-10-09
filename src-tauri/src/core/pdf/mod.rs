use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::models::PdfParseResult;

const MAX_PDF_PAGE_WARNING: u32 = 100;

pub struct PdfProcessor;

impl PdfProcessor {
    pub fn extract_text(path: &Path) -> Result<PdfParseResult> {
        let bytes = std::fs::read(path).context("Failed to read PDF file")?;
        let hash = Self::compute_hash(&bytes);

        let content = pdf_extract::extract_text_from_mem(&bytes)
            .context("Failed to extract text from PDF")?;

        let page_count = Self::estimate_page_count(&bytes);

        if page_count > MAX_PDF_PAGE_WARNING {
            tracing::warn!(
                "PDF has {} pages, performance may be degraded",
                page_count
            );
        }

        let warning = if page_count > MAX_PDF_PAGE_WARNING {
            Some(format!("PDF {} sayfa. Çok uzun PDF'lerde performans düşebilir.", page_count))
        } else {
            None
        };

        let suggested_query = build_suggested_query(&content);

        Ok(PdfParseResult {
            text: content,
            page_count,
            hash,
            warning,
            suggested_query: Some(suggested_query),
        })
    }

    pub fn page_count(path: &Path) -> Result<u32> {
        let bytes = std::fs::read(path).context("Failed to read PDF file")?;
        Ok(Self::estimate_page_count(&bytes))
    }

    pub fn validate(path: &Path) -> Result<bool> {
        let bytes = std::fs::read(path).context("Failed to read PDF file")?;
        if bytes.len() < 5 {
            return Ok(false);
        }
        let header = &bytes[..5];
        Ok(header == b"%PDF-")
    }

    fn compute_hash(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    fn estimate_page_count(bytes: &[u8]) -> u32 {
        let text = String::from_utf8_lossy(bytes);
        let count = text.matches("/Type /Page").count();
        if count > 0 {
            count as u32
        } else {
            1
        }
    }
}

fn build_suggested_query(text: &str) -> String {
    const MAX_CHARS: usize = 400;
    let normalized: String = text
        .split_whitespace()
        .take(80)
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.chars().count() <= MAX_CHARS {
        normalized
    } else {
        normalized.chars().take(MAX_CHARS).collect()
    }
}
