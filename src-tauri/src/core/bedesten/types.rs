#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};

// ---- Request types ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenSearchRequest {
    pub data: BedestenSearchData,
    pub applicationName: String,
    pub paging: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenSearchData {
    pub pageSize: u32,
    pub pageNumber: u32,
    pub itemTypeList: Vec<String>,
    pub phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birimAdi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kararTarihiStart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kararTarihiEnd: Option<String>,
    pub sortFields: Vec<String>,
    pub sortDirection: String,
}

// ---- Raw API Response types ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenSearchRawResponse {
    pub data: Option<BedestenSearchDataResponse>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenSearchDataResponse {
    pub emsalKararList: Vec<BedestenDecisionEntry>,
    pub total: i64,
    pub start: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenDecisionEntry {
    pub documentId: String,
    pub itemType: Option<BedestenItemType>,
    pub birimAdi: Option<String>,
    pub esasNo: Option<String>,
    pub kararNo: Option<String>,
    pub kararTarihiStr: Option<String>,
    pub kararTarihi: Option<String>,
    pub esasNoYil: Option<i32>,
    pub esasNoSira: Option<i32>,
    pub kararNoYil: Option<i32>,
    pub kararNoSira: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenItemType {
    pub name: String,
    pub description: Option<String>,
}

// ---- CLI-friendly output types ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenSearchOutput {
    pub decisions: Vec<BedestenDecisionEntry>,
    pub totalRecords: i64,
    pub requestedPage: u32,
    pub pageSize: u32,
    pub searchedCourts: Vec<String>,
}

// ---- Document request/response ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenDocumentRequest {
    pub data: BedestenDocumentRequestData,
    pub applicationName: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenDocumentRequestData {
    pub documentId: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenDocumentRawResponse {
    pub data: Option<BedestenDocumentContent>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenDocumentContent {
    pub content: Option<String>,
    pub mimeType: Option<String>,
    pub version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedestenDocumentMarkdown {
    pub documentId: String,
    pub markdownContent: String,
    pub sourceUrl: String,
    pub mimeType: String,
}
