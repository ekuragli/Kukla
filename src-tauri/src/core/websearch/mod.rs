//! İnternet arama — Tavily sağlayıcısı.
//!
//! Ücretsiz katman: ayda 1.000 arama kredisi, kayıt olmadan kart istenmez.
//! API anahtarı Ayarlar → YZ Altyapısı bölümünden girilir ve şifreli SQLite'da
//! saklanır. KVKK: bu modda kullanıcının sorusu Tavily'ye gönderilir; bu nedenle
//! yalnızca sohbet oturumunda paylaşıma açıkça izin verildiğinde çalışır.

use serde::{Deserialize, Serialize};

pub const TAVILY_SEARCH_URL: &str = "https://api.tavily.com/search";
pub const TAVILY_FREE_MONTHLY_CREDITS: u32 = 1_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub content: String,
    #[serde(default)]
    pub score: f64,
}

/// Arama derinliği: `basic` hızlı ve tek kredi, `advanced` daha derin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDepth {
    Basic,
    Advanced,
}

impl SearchDepth {
    pub fn as_str(&self) -> &'static str {
        match self {
            SearchDepth::Basic => "basic",
            SearchDepth::Advanced => "advanced",
        }
    }

    pub fn from_optional(value: Option<&str>) -> Self {
        match value {
            Some("advanced") => SearchDepth::Advanced,
            _ => SearchDepth::Basic,
        }
    }
}

pub struct TavilyProvider {
    api_key: String,
    http: reqwest::Client,
}

impl TavilyProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            api_key: api_key.into(),
            http,
        }
    }

    pub async fn search(
        &self,
        query: &str,
        max_results: u32,
        depth: SearchDepth,
    ) -> Result<Vec<WebSearchResult>, String> {
        let payload = build_request_payload(query, max_results, depth);
        let response = self
            .http
            .post(TAVILY_SEARCH_URL)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Tavily sunucusuna ulaşılamadı: {}", e))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| format!("Tavily yanıtı okunamadı: {}", e))?;

        if !status.is_success() {
            return Err(tavily_error_message(status.as_u16(), &body));
        }

        parse_search_response(&body)
    }
}

/// İstek gövdesi (ağ olmadan test edilebilir).
pub fn build_request_payload(query: &str, max_results: u32, depth: SearchDepth) -> serde_json::Value {
    serde_json::json!({
        "query": query,
        "max_results": max_results.clamp(1, 10),
        "search_depth": depth.as_str(),
        "include_answer": false,
    })
}

#[derive(Deserialize)]
struct TavilyResponse {
    #[serde(default)]
    results: Vec<TavilyItem>,
}

#[derive(Deserialize)]
struct TavilyItem {
    title: Option<String>,
    url: Option<String>,
    content: Option<String>,
    #[serde(default)]
    score: f64,
}

/// Tavily JSON yanıtını iç sonuçlara çevirir.
pub fn parse_search_response(body: &str) -> Result<Vec<WebSearchResult>, String> {
    let parsed: TavilyResponse =
        serde_json::from_str(body).map_err(|e| format!("Tavily yanıtı çözümlenemedi: {}", e))?;

    let results = parsed
        .results
        .into_iter()
        .filter_map(|item| {
            let url = item.url?;
            Some(WebSearchResult {
                title: item.title.unwrap_or_else(|| url.clone()),
                url,
                content: item.content.unwrap_or_default(),
                score: item.score,
            })
        })
        .collect();

    Ok(results)
}

/// HTTP durumunu kullanıcı-dostu Türkçe hataya çevirir.
pub fn tavily_error_message(status: u16, body: &str) -> String {
    let detail = first_line(body);
    match status {
        401 | 403 => "Tavily API anahtarı geçersiz. Ayarlar → YZ Altyapısı bölümünden kontrol edin."
            .to_string(),
        429 => format!(
            "Tavily aylık ücretsiz kredi sınırına ulaşıldı ({} arama/ay).",
            TAVILY_FREE_MONTHLY_CREDITS
        ),
        _ => format!("Tavily arama hatası (HTTP {}): {}", status, detail),
    }
}

fn first_line(body: &str) -> String {
    body.lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("boş yanıt")
        .chars()
        .take(200)
        .collect()
}
