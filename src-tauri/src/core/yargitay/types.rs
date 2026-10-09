#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct YargitaySimpleSearchData {
    pub aranan: String,
    pub arananKelime: String,
    pub pageSize: u32,
    pub pageNumber: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YargitayDetailSearchData {
    pub arananKelime: String,
    pub birimYrgKurulDaire: String,
    pub birimYrgHukukDaire: String,
    pub birimYrgCezaDaire: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub baslangicTarihi: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub bitisTarihi: String,
    pub pageSize: u32,
    pub pageNumber: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct YargitaySearchRequest<T> {
    pub data: T,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YargitaySearchRawResponse {
    pub data: Option<YargitaySearchInnerData>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YargitaySearchInnerData {
    pub data: Vec<YargitayDecisionEntry>,
    pub recordsTotal: i64,
    pub recordsFiltered: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YargitayDecisionEntry {
    pub id: String,
    pub daire: Option<String>,
    pub esasNo: Option<String>,
    pub kararNo: Option<String>,
    pub kararTarihi: Option<String>,
    pub arananKelime: Option<String>,
}

#[derive(Debug, Clone)]
pub struct YargitaySearchOutput {
    pub decisions: Vec<YargitayDecisionEntry>,
    pub total_records: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YargitayDocumentRawResponse {
    pub data: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct YargitayDocument {
    pub document_id: String,
    pub text: String,
    pub source_url: String,
}