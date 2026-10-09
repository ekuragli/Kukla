use anyhow::Result;
use std::path::Path;

use crate::models::ChunkMetadata;
use crate::core::rag::vector_search::{Filters, SearchHit, VectorSearchTrait, InMemoryVectorSearch};

pub enum VectorDb {
    Persistent {
        store: InMemoryVectorSearch,
        data_dir: std::path::PathBuf,
    },
    InMemory(InMemoryVectorSearch),
}

impl VectorDb {
    pub fn new_in_memory() -> Self {
        VectorDb::InMemory(InMemoryVectorSearch::new())
    }

    pub fn connect(data_dir: &Path) -> Result<Self> {
        let store_path = data_dir.join("vectors.bin");
        let store = InMemoryVectorSearch::load(&store_path);
        tracing::info!(
            "Vector store loaded: {} chunks from {}",
            store.count().unwrap_or(0),
            store_path.display()
        );
        Ok(VectorDb::Persistent {
            store,
            data_dir: data_dir.to_path_buf(),
        })
    }

    fn persist_if_needed(&self, result: Result<u64>) -> Result<u64> {
        if result.is_ok() {
            if let Err(e) = self.save() {
                tracing::warn!("Vector store save failed: {}", e);
            }
        }
        result
    }

    pub fn save(&self) -> Result<()> {
        match self {
            VectorDb::Persistent { store, data_dir } => {
                std::fs::create_dir_all(data_dir)?;
                let store_path = data_dir.join("vectors.bin");
                store.save(&store_path)?;
                Ok(())
            }
            VectorDb::InMemory(_) => Ok(()),
        }
    }

    pub fn search(
        &self,
        embedding: &[f32],
        top_k: u32,
        filters: Option<&Filters>,
    ) -> Result<Vec<SearchHit>> {
        match self {
            VectorDb::Persistent { store, .. } => store.search(embedding, top_k, filters),
            VectorDb::InMemory(store) => store.search(embedding, top_k, filters),
        }
    }

    pub fn insert_chunk(&self, id: &str, embedding: &[f32], metadata: &ChunkMetadata) -> Result<()> {
        let result = match self {
            VectorDb::Persistent { store, .. } => store.insert_chunk(id, embedding, metadata),
            VectorDb::InMemory(store) => store.insert_chunk(id, embedding, metadata),
        };
        if result.is_ok() {
            if let Err(e) = self.save() {
                tracing::warn!("Vector store save failed: {}", e);
            }
        }
        result
    }

    pub fn delete_by_source(&self, source: &str) -> Result<u64> {
        let result = match self {
            VectorDb::Persistent { store, .. } => store.delete_chunks(source),
            VectorDb::InMemory(store) => store.delete_chunks(source),
        };
        self.persist_if_needed(result)
    }

    pub fn count(&self) -> Result<u64> {
        match self {
            VectorDb::Persistent { store, .. } => store.count(),
            VectorDb::InMemory(store) => store.count(),
        }
    }

    pub fn list_chunks(&self, source_filter: &str) -> Result<Vec<crate::models::ChunkMetadata>> {
        match self {
            VectorDb::Persistent { store, .. } => store.list_chunks(source_filter),
            VectorDb::InMemory(store) => store.list_chunks(source_filter),
        }
    }

    pub fn delete_chunk(&self, chunk_id: &str) -> Result<u64> {
        let result = match self {
            VectorDb::Persistent { store, .. } => store.delete_chunk(chunk_id),
            VectorDb::InMemory(store) => store.delete_chunk(chunk_id),
        };
        self.persist_if_needed(result)
    }

    pub fn delete_chunk_by_decision_id(&self, decision_id: i64) -> Result<u64> {
        let result = match self {
            VectorDb::Persistent { store, .. } => store.delete_chunk_by_decision_id(decision_id),
            VectorDb::InMemory(store) => store.delete_chunk_by_decision_id(decision_id),
        };
        self.persist_if_needed(result)
    }
}