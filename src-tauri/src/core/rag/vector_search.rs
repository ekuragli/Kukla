use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use crate::models::ChunkMetadata;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: String,
    pub score: f64,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filters {
    pub source: Option<String>,
    pub daire: Option<String>,
    pub year: Option<i32>,
    pub year_end: Option<i32>,
}

pub trait VectorSearchTrait: Send + Sync {
    fn search(
        &self,
        embedding: &[f32],
        top_k: u32,
        filters: Option<&Filters>,
    ) -> Result<Vec<SearchHit>>;

    fn insert_chunk(&self, id: &str, embedding: &[f32], metadata: &ChunkMetadata) -> Result<()>;

    fn delete_chunks(&self, source_filter: &str) -> Result<u64>;

    fn delete_chunk(&self, chunk_id: &str) -> Result<u64>;

    fn delete_chunk_by_decision_id(&self, decision_id: i64) -> Result<u64>;

    fn list_chunks(&self, source_filter: &str) -> Result<Vec<ChunkMetadata>>;

    fn count(&self) -> Result<u64>;
}

pub struct InMemoryVectorSearch {
    chunks: Mutex<Vec<(String, Vec<f32>, ChunkMetadata)>>,
}

impl Default for InMemoryVectorSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryVectorSearch {
    pub fn new() -> Self {
        InMemoryVectorSearch { chunks: Mutex::new(Vec::new()) }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let chunks = self.chunks.lock().unwrap();
        let data = bincode::serialize(&*chunks).context("Vektör verisi serileştirilemedi")?;
        std::fs::write(path, &data).context("Vektör verisi dosyaya yazılamadı")?;
        Ok(())
    }

    pub fn load(path: &Path) -> Self {
        let chunks: Vec<(String, Vec<f32>, ChunkMetadata)> = match std::fs::read(path) {
            Ok(data) => bincode::deserialize(&data).unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        // Embedding modeli değiştiğinde (ör. 1024 -> 384 boyut) eski vektörler
        // kullanılamaz; tamamı atılır ve indeks sıfırdan yeniden kurulur.
        let expected = crate::core::embedding::EMBEDDING_DIM;
        let stale = chunks.iter().filter(|(_, emb, _)| emb.len() != expected).count();
        let chunks = if stale > 0 && stale == chunks.len() {
            tracing::warn!(
                "Vector store sıfırlandı: {} vektörün boyutu {}, beklenen {}.",
                stale,
                chunks
                    .first()
                    .map(|(_, emb, _)| emb.len())
                    .unwrap_or(0),
                expected
            );
            let _ = std::fs::remove_file(path);
            Vec::new()
        } else if stale > 0 {
            tracing::warn!(
                "Vector store: boyutu eşleşmeyen {} vektör atlandı.",
                stale
            );
            chunks
                .into_iter()
                .filter(|(_, emb, _)| emb.len() == expected)
                .collect()
        } else {
            chunks
        };

        InMemoryVectorSearch { chunks: Mutex::new(chunks) }
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        (dot / (norm_a * norm_b)) as f64
    }
}

impl VectorSearchTrait for InMemoryVectorSearch {
    fn search(
        &self,
        embedding: &[f32],
        top_k: u32,
        filters: Option<&Filters>,
    ) -> Result<Vec<SearchHit>> {
        let chunks = self.chunks.lock().unwrap();
        let mut scored: Vec<SearchHit> = chunks
            .iter()
            .filter(|(_, _, meta)| {
                if let Some(f) = filters {
                    if let Some(ref source) = f.source {
                        if meta.source != *source {
                            return false;
                        }
                    }
                    if let Some(ref daire) = f.daire {
                        if meta.daire.as_deref() != Some(daire) {
                            return false;
                        }
                    }
                    if let Some(filter_year) = f.year {
                        if let Some(chunk_date) = meta.karar_tarihi {
                            if chunk_date.format("%Y").to_string().parse::<i32>().unwrap_or(0) < filter_year {
                                return false;
                            }
                        }
                    }
                    if let Some(filter_year_end) = f.year_end {
                        if let Some(chunk_date) = meta.karar_tarihi {
                            if chunk_date.format("%Y").to_string().parse::<i32>().unwrap_or(9999) > filter_year_end {
                                return false;
                            }
                        }
                    }
                }
                true
            })
            .map(|(id, emb, meta)| SearchHit {
                id: id.clone(),
                score: Self::cosine_similarity(embedding, emb),
                metadata: meta.clone(),
            })
            .collect();

        drop(chunks);
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k as usize);

        Ok(scored)
    }

    fn insert_chunk(&self, id: &str, embedding: &[f32], metadata: &ChunkMetadata) -> Result<()> {
        let mut chunks = self.chunks.lock().unwrap();
        chunks.push((id.to_string(), embedding.to_vec(), metadata.clone()));
        Ok(())
    }

    fn delete_chunks(&self, source_filter: &str) -> Result<u64> {
        let mut chunks = self.chunks.lock().unwrap();
        let before = chunks.len();
        chunks.retain(|(_, _, meta)| meta.source != source_filter);
        Ok((before - chunks.len()) as u64)
    }

    fn delete_chunk(&self, chunk_id: &str) -> Result<u64> {
        let mut chunks = self.chunks.lock().unwrap();
        let before = chunks.len();
        chunks.retain(|(id, _, _)| id != chunk_id);
        Ok((before - chunks.len()) as u64)
    }

    fn delete_chunk_by_decision_id(&self, decision_id: i64) -> Result<u64> {
        let mut chunks = self.chunks.lock().unwrap();
        let before = chunks.len();
        chunks.retain(|(_, _, meta)| {
            crate::utils::chunk_meta::decision_id_from_chunk(meta) != Some(decision_id)
        });
        Ok((before - chunks.len()) as u64)
    }

    fn list_chunks(&self, source_filter: &str) -> Result<Vec<ChunkMetadata>> {
        let chunks = self.chunks.lock().unwrap();
        let result: Vec<ChunkMetadata> = chunks
            .iter()
            .filter(|(_, _, meta)| meta.source == source_filter)
            .map(|(_, _, meta)| meta.clone())
            .collect();
        Ok(result)
    }

    fn count(&self) -> Result<u64> {
        let chunks = self.chunks.lock().unwrap();
        Ok(chunks.len() as u64)
    }
}


