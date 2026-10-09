use anyhow::{Context, Result};
use std::time::Duration;

use super::types::*;

const SEARCH_TIMEOUT_SECS: u64 = 90;

pub struct YargitayClient {
    client: reqwest::Client,
    base_url: String,
}

impl Default for YargitayClient {
    fn default() -> Self {
        Self::new()
    }
}

impl YargitayClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(SEARCH_TIMEOUT_SECS))
            .cookie_store(true)
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert(
                    reqwest::header::CONTENT_TYPE,
                    reqwest::header::HeaderValue::from_static(
                        "application/json; charset=UTF-8",
                    ),
                );
                h.insert(
                    reqwest::header::ACCEPT,
                    reqwest::header::HeaderValue::from_static("application/json, text/plain, */*"),
                );
                h.insert(
                    "X-Requested-With",
                    reqwest::header::HeaderValue::from_static("XMLHttpRequest"),
                );
                h.insert(
                    "X-KL-KIS-Ajax-Request",
                    reqwest::header::HeaderValue::from_static("Ajax_Request"),
                );
                h.insert(
                    reqwest::header::ORIGIN,
                    reqwest::header::HeaderValue::from_static("https://karararama.yargitay.gov.tr"),
                );
                h.insert(
                    reqwest::header::REFERER,
                    reqwest::header::HeaderValue::from_static("https://karararama.yargitay.gov.tr/"),
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
            .expect("Failed to build Yargıtay HTTP client");

        YargitayClient {
            client,
            base_url: "https://karararama.yargitay.gov.tr".to_string(),
        }
    }

    async fn ensure_session(&self) -> Result<()> {
        self.client
            .get(format!("{}/", self.base_url))
            .send()
            .await
            .context("Yargıtay oturumu başlatılamadı")?;
        Ok(())
    }

    pub async fn search_decisions(
        &self,
        phrase: &str,
        _hukuk_daire: Option<&str>,
        _ceza_daire: Option<&str>,
        _kurul: Option<&str>,
        _year_start: Option<i32>,
        _year_end: Option<i32>,
        page: u32,
        page_size: u32,
        max_pages: u32,
    ) -> Result<YargitaySearchOutput> {
        const MAX_RETRIES: u32 = 5;
        let page_count = max_pages.max(1);

        let mut all_decisions = Vec::new();
        let mut total_records = 0i64;
        let mut last_error: Option<anyhow::Error> = None;

        for (page_index, page_num) in (page..page + page_count).enumerate() {
            if page_index > 0 {
                tokio::time::sleep(Duration::from_millis(600)).await;
            }

            let mut page_result = None;
            for attempt in 0..MAX_RETRIES {
                if attempt > 0 {
                    self.ensure_session().await.ok();
                    tokio::time::sleep(Duration::from_secs(1 + 2u64.pow(attempt))).await;
                }

                match self.search_simple(phrase, page_num, page_size).await {
                    Ok(output) => {
                        page_result = Some(output);
                        last_error = None;
                        break;
                    }
                    Err(e) => {
                        last_error = Some(e);
                    }
                }
            }

            let Some(output) = page_result else {
                if all_decisions.is_empty() {
                    return Err(last_error.unwrap_or_else(|| {
                        anyhow::anyhow!("Yargıtay aramasından sonuç alınamadı.")
                    }));
                }
                break;
            };

            total_records = output.total_records;
            if output.decisions.is_empty() {
                break;
            }
            all_decisions.extend(output.decisions);
            if all_decisions.len() as i64 >= total_records {
                break;
            }
        }

        if all_decisions.is_empty() {
            return Err(last_error.unwrap_or_else(|| {
                anyhow::anyhow!("Yargıtay aramasından sonuç alınamadı.")
            }));
        }

        Ok(YargitaySearchOutput {
            decisions: all_decisions,
            total_records,
        })
    }

    async fn search_simple(
        &self,
        phrase: &str,
        page: u32,
        page_size: u32,
    ) -> Result<YargitaySearchOutput> {
        self.ensure_session().await?;

        let warmup = YargitaySearchRequest {
            data: serde_json::json!({
                "aranan": phrase,
                "arananKelime": phrase
            }),
        };

        let arama_resp = self
            .client
            .post(format!("{}/arama", self.base_url))
            .json(&warmup)
            .send()
            .await
            .context("Yargıtay arama oturumu hazırlanamadı")?;
        let _ = arama_resp.text().await;

        let request = YargitaySearchRequest {
            data: YargitaySimpleSearchData {
                aranan: phrase.to_string(),
                arananKelime: phrase.to_string(),
                pageSize: page_size.min(10),
                pageNumber: page,
            },
        };

        self.post_search_list(&request, "/aramalist").await
    }

    async fn post_search_list<T: serde::Serialize>(
        &self,
        request: &YargitaySearchRequest<T>,
        endpoint: &str,
    ) -> Result<YargitaySearchOutput> {
        let response = self
            .client
            .post(format!("{}{}", self.base_url, endpoint))
            .json(request)
            .send()
            .await
            .with_context(|| format!("Yargıtay arama isteği başarısız ({endpoint})"))?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(anyhow::anyhow!(
                "Yargıtay API rate limit aşıldı. Lütfen birkaç saniye bekleyin."
            ));
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Yargıtay arama hatası: HTTP {} — {}",
                status,
                crate::utils::truncate_text(&body, 200)
            ));
        }

        let raw: YargitaySearchRawResponse = response
            .json()
            .await
            .context("Yargıtay arama yanıtı ayrıştırılamadı")?;

        if let Some(data) = raw.data {
            return Ok(YargitaySearchOutput {
                decisions: data.data,
                total_records: data.recordsFiltered,
            });
        }

        Err(anyhow::anyhow!(
            "Yargıtay arama hatası: {}",
            api_error_message(&raw.metadata)
        ))
    }

    pub async fn get_document(&self, document_id: &str) -> Result<YargitayDocument> {
        self.ensure_session().await?;

        let response = self
            .client
            .get(format!("{}/getDokuman?id={}", self.base_url, document_id))
            .send()
            .await
            .context("Yargıtay döküman API'sine bağlanılamadı")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Yargıtay döküman hatası: HTTP {}",
                response.status()
            ));
        }

        let raw: YargitayDocumentRawResponse = response
            .json()
            .await
            .context("Yargıtay döküman yanıtı ayrıştırılamadı")?;

        let html = raw.data.ok_or_else(|| {
            anyhow::anyhow!(
                "Döküman içeriği bulunamadı: {}",
                api_error_message(&raw.metadata)
            )
        })?;

        Ok(YargitayDocument {
            document_id: document_id.to_string(),
            text: html_to_text(&html),
            source_url: public_viewer_url(document_id),
        })
    }
}

fn api_error_message(metadata: &Option<serde_json::Value>) -> String {
    metadata
        .as_ref()
        .and_then(|m| m.get("FMTE").or_else(|| m.get("FMU")))
        .and_then(|v| v.as_str())
        .unwrap_or("Bilinmeyen API hatası")
        .to_string()
}

pub fn public_viewer_url(_document_id: &str) -> String {
    "https://karararama.yargitay.gov.tr/".to_string()
}

pub fn map_daire_code(code: &str) -> (Option<String>, Option<String>, Option<String>) {
    match code {
        "HGK" => (None, None, Some("Hukuk Genel Kurulu".to_string())),
        "CGK" => (None, None, Some("Ceza Genel Kurulu".to_string())),
        "BGK" => (None, None, Some("Büyük Genel Kurulu".to_string())),
        "HCBK" => (
            None,
            None,
            Some("Hukuk Daireleri Başkanlar Kurulu".to_string()),
        ),
        "CCBK" => (
            None,
            None,
            Some("Ceza Daireleri Başkanlar Kurulu".to_string()),
        ),
        code if code.starts_with('H') => {
            let num = code.trim_start_matches('H');
            (
                Some(format!("{num}. Hukuk Dairesi")),
                None,
                None,
            )
        }
        code if code.starts_with('C') => {
            let num = code.trim_start_matches('C');
            (
                None,
                Some(format!("{num}. Ceza Dairesi")),
                None,
            )
        }
        other if other.contains("Hukuk Dairesi") => (Some(other.to_string()), None, None),
        other if other.contains("Ceza Dairesi") => (None, Some(other.to_string()), None),
        other if other.contains("Kurulu") => (None, None, Some(other.to_string())),
        _ => (None, None, None),
    }
}

fn html_to_text(html: &str) -> String {
    let mut decoded = html.to_string();
    if decoded.contains("\\\"") || decoded.contains("\\n") {
        decoded = decoded
            .replace("\\\"", "\"")
            .replace("\\r\\n", "\n")
            .replace("\\n", "\n")
            .replace("\\t", "\t");
    }

    let mut text = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let chars: Vec<char> = decoded.chars().collect();
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