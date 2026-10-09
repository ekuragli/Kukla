use anyhow::{Context, Result};
use base64::Engine;
use std::time::Duration;

use super::types::*;

pub struct BedestenClient {
    client: reqwest::Client,
    base_url: String,
}

impl Default for BedestenClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BedestenClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert(
                    "AdaletApplicationName",
                    reqwest::header::HeaderValue::from_static("UyapMevzuat"),
                );
                h.insert(
                    reqwest::header::CONTENT_TYPE,
                    reqwest::header::HeaderValue::from_static("application/json; charset=utf-8"),
                );
                h.insert(
                    reqwest::header::ORIGIN,
                    reqwest::header::HeaderValue::from_static("https://mevzuat.adalet.gov.tr"),
                );
                h.insert(
                    reqwest::header::REFERER,
                    reqwest::header::HeaderValue::from_static("https://mevzuat.adalet.gov.tr/"),
                );
                h.insert(
                    reqwest::header::ACCEPT,
                    reqwest::header::HeaderValue::from_static("*/*"),
                );
                h.insert(
                    reqwest::header::USER_AGENT,
                    reqwest::header::HeaderValue::from_static(
                        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36",
                    ),
                );
                h
            })
            .build()
            .expect("Failed to build HTTP client");

        BedestenClient {
            client,
            base_url: "https://bedesten.adalet.gov.tr".to_string(),
        }
    }

    pub async fn search_documents(
        &self,
        phrase: &str,
        court_types: Option<Vec<String>>,
        birim_adi: Option<&str>,
        year_start: Option<i32>,
        year_end: Option<i32>,
        page: u32,
    ) -> Result<BedestenSearchOutput> {
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 0..max_retries {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
            }

            let data = BedestenSearchData {
                pageSize: 10,
                pageNumber: page,
                itemTypeList: court_types.clone().unwrap_or_else(|| vec!["YARGITAYKARARI".to_string(), "DANISTAYKARAR".to_string()]),
                phrase: phrase.to_string(),
                birimAdi: birim_adi.map(map_chamber_code).filter(|s| s != "ALL" && !s.is_empty()),
                kararTarihiStart: year_start.map(|y| format!("01.01.{y}")),
                kararTarihiEnd: year_end.map(|y| format!("31.12.{y}")),
                sortFields: vec![],
                sortDirection: "desc".to_string(),
            };

            let request = BedestenSearchRequest {
                data,
                applicationName: "UyapMevzuat".to_string(),
                paging: true,
            };

            let response = match self
                .client
                .post(format!("{}/emsal-karar/searchDocuments", self.base_url))
                .json(&request)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(anyhow::anyhow!("Bedesten API'ye bağlanılamadı: {}", e));
                    continue;
                }
            };

            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(anyhow::anyhow!("Bedesten API rate limit aşıldı. Lütfen birkaç saniye bekleyin."));
            }

            let raw: BedestenSearchRawResponse = match response.json().await {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(anyhow::anyhow!("Bedesten yanıtı ayrıştırılamadı: {}", e));
                    continue;
                }
            };

            if let Some(data_resp) = raw.data {
                return Ok(BedestenSearchOutput {
                    decisions: data_resp.emsalKararList,
                    totalRecords: data_resp.total,
                    requestedPage: page,
                    pageSize: 10,
                    searchedCourts: vec![],
                });
            }

            let err_msg = raw
                .metadata
                .as_ref()
                .and_then(|m| m.get("FMTE"))
                .and_then(|v| v.as_str())
                .unwrap_or("Bilinmeyen hata");

            if err_msg.contains("IOException") {
                last_error = Some(anyhow::anyhow!("Bedesten API hatası: {} (deneme {}/{})", err_msg, attempt + 1, max_retries));
                continue;
            }

            return Err(anyhow::anyhow!("Bedesten API hatası: {}", err_msg));
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Bedesten API hatası: Bilinmeyen hata")))
    }

    pub async fn get_document_markdown(&self, document_id: &str) -> Result<BedestenDocumentMarkdown> {
        let request = BedestenDocumentRequest {
            data: BedestenDocumentRequestData {
                documentId: document_id.to_string(),
            },
            applicationName: "UyapMevzuat".to_string(),
        };

        let response = self
            .client
            .post(format!("{}/emsal-karar/getDocumentContent", self.base_url))
            .json(&request)
            .send()
            .await
            .context("Bedesten API'ye bağlanılamadı (döküman)")?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(anyhow::anyhow!("Bedesten API rate limit aşıldı."));
        }

        let raw: BedestenDocumentRawResponse = response
            .json()
            .await
            .context("Bedesten döküman yanıtı ayrıştırılamadı")?;

        let doc_content = raw
            .data
            .ok_or_else(|| anyhow::anyhow!("Döküman içeriği bulunamadı"))?;

        let content = doc_content
            .content
            .ok_or_else(|| anyhow::anyhow!("Döküman içerik alanı boş"))?;

        let mime_type = doc_content.mimeType.unwrap_or_default();

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&content)
            .context("Base64 çözümleme hatası")?;

        let markdown_content = if mime_type.contains("html") {
            simple_html_to_text(&String::from_utf8_lossy(&decoded))
        } else {
            String::from_utf8_lossy(&decoded).to_string()
        };

        Ok(BedestenDocumentMarkdown {
            documentId: document_id.to_string(),
            markdownContent: markdown_content,
            sourceUrl: format!("https://mevzuat.adalet.gov.tr/ictihat/{}", document_id),
            mimeType: mime_type,
        })
    }
}

fn map_chamber_code(code: &str) -> String {
    match code {
        "H1" => "1. Hukuk Dairesi".to_string(),
        "H2" => "2. Hukuk Dairesi".to_string(),
        "H3" => "3. Hukuk Dairesi".to_string(),
        "H4" => "4. Hukuk Dairesi".to_string(),
        "H5" => "5. Hukuk Dairesi".to_string(),
        "H6" => "6. Hukuk Dairesi".to_string(),
        "H7" => "7. Hukuk Dairesi".to_string(),
        "H8" => "8. Hukuk Dairesi".to_string(),
        "H9" => "9. Hukuk Dairesi".to_string(),
        "H10" => "10. Hukuk Dairesi".to_string(),
        "H11" => "11. Hukuk Dairesi".to_string(),
        "H12" => "12. Hukuk Dairesi".to_string(),
        "H13" => "13. Hukuk Dairesi".to_string(),
        "H14" => "14. Hukuk Dairesi".to_string(),
        "H15" => "15. Hukuk Dairesi".to_string(),
        "H16" => "16. Hukuk Dairesi".to_string(),
        "H17" => "17. Hukuk Dairesi".to_string(),
        "H18" => "18. Hukuk Dairesi".to_string(),
        "H19" => "19. Hukuk Dairesi".to_string(),
        "H20" => "20. Hukuk Dairesi".to_string(),
        "H21" => "21. Hukuk Dairesi".to_string(),
        "H22" => "22. Hukuk Dairesi".to_string(),
        "H23" => "23. Hukuk Dairesi".to_string(),
        "C1" => "1. Ceza Dairesi".to_string(),
        "C2" => "2. Ceza Dairesi".to_string(),
        "C3" => "3. Ceza Dairesi".to_string(),
        "C4" => "4. Ceza Dairesi".to_string(),
        "C5" => "5. Ceza Dairesi".to_string(),
        "C6" => "6. Ceza Dairesi".to_string(),
        "C7" => "7. Ceza Dairesi".to_string(),
        "C8" => "8. Ceza Dairesi".to_string(),
        "C9" => "9. Ceza Dairesi".to_string(),
        "C10" => "10. Ceza Dairesi".to_string(),
        "C11" => "11. Ceza Dairesi".to_string(),
        "C12" => "12. Ceza Dairesi".to_string(),
        "C13" => "13. Ceza Dairesi".to_string(),
        "C14" => "14. Ceza Dairesi".to_string(),
        "C15" => "15. Ceza Dairesi".to_string(),
        "C16" => "16. Ceza Dairesi".to_string(),
        "C17" => "17. Ceza Dairesi".to_string(),
        "C18" => "18. Ceza Dairesi".to_string(),
        "C19" => "19. Ceza Dairesi".to_string(),
        "C20" => "20. Ceza Dairesi".to_string(),
        "C21" => "21. Ceza Dairesi".to_string(),
        "C22" => "22. Ceza Dairesi".to_string(),
        "C23" => "23. Ceza Dairesi".to_string(),
        "HGK" => "Hukuk Genel Kurulu".to_string(),
        "CGK" => "Ceza Genel Kurulu".to_string(),
        _ => code.to_string(),
    }
}

fn simple_html_to_text(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    let mut in_script = false;

    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '<' {
            in_tag = true;
            let rest: String = chars.iter().skip(i).take(20).collect();
            let rest_lower = rest.to_lowercase();
            if rest_lower.starts_with("<script") {
                in_script = true;
            } else if rest_lower.starts_with("</script") {
                in_script = false;
            }
            i += 1;
            continue;
        }

        if chars[i] == '>' && in_tag {
            in_tag = false;
            i += 1;
            text.push('\n');
            continue;
        }

        if !in_tag && !in_script {
            text.push(chars[i]);
        }

        i += 1;
    }

    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
