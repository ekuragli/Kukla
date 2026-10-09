//! Yerel embedding — Candle üzerinde `sentence-transformers/all-MiniLM-L6-v2`.
//!
//! * Boyut: **384** (`EMBEDDING_DIM`)
//! * İlk kullanımda model dosyaları indirilir: `~/.cache/kukla/models/all-MiniLM-L6-v2`
//!   (Windows: `%LOCALAPPDATA%\kukla\models\all-MiniLM-L6-v2`)
//! * Tüm işlem CPU üzerinde çalışır; internet yalnızca ilk indirmede gerekir.

use std::path::{Path, PathBuf};

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use tokenizers::{PaddingParams, PaddingStrategy, TruncationParams, TruncationStrategy};

/// Vektör boyutu. Vector store bu boyutla çalışır; değişirse indeks sıfırlanır.
pub const EMBEDDING_DIM: usize = 384;
pub const MODEL_NAME: &str = "all-MiniLM-L6-v2";
pub const MODEL_REPO: &str = "sentence-transformers/all-MiniLM-L6-v2";

/// BERT için güvenli token sınırı (512 yerine 256 yeterli).
const MAX_TOKENS: usize = 256;
const BATCH_SIZE: usize = 16;
const NORM_EPSILON: f32 = 1e-12;
const MODEL_FILES: [&str; 3] = ["config.json", "tokenizer.json", "model.safetensors"];

pub struct Embedder {
    tokenizer: tokenizers::Tokenizer,
    model: BertModel,
    device: Device,
    dim: usize,
}

impl Embedder {
    /// Model dosyalarının tutulduğu dizin.
    pub fn model_dir() -> PathBuf {
        dirs_next::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("kukla")
            .join("models")
            .join(MODEL_NAME)
    }

    /// İndirilmemiş model dosyalarını indirir. Var olan dosyaya dokunmaz.
    pub async fn download_missing() -> Result<PathBuf, String> {
        let dir = Self::model_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Model dizini oluşturulamadı: {}", e))?;

        for file in MODEL_FILES {
            let dest = dir.join(file);
            if is_non_empty(&dest) {
                continue;
            }

            let url = format!(
                "https://huggingface.co/{}/resolve/main/{}",
                MODEL_REPO, file
            );
            let response = reqwest::get(&url)
                .await
                .map_err(|e| format!("Model indirilemedi ({}): {}", file, e))?
                .error_for_status()
                .map_err(|e| format!("Model indirilemedi ({}): {}", file, e))?;
            let bytes = response
                .bytes()
                .await
                .map_err(|e| format!("Model indirilemedi ({}): {}", file, e))?;
            if bytes.is_empty() {
                return Err(format!("İndirilen dosya boş: {}", file));
            }

            let temp = dir.join(format!("{}.part", file));
            std::fs::write(&temp, &bytes)
                .map_err(|e| format!("Model dosyası yazılamadı ({}): {}", file, e))?;
            std::fs::rename(&temp, &dest)
                .map_err(|e| format!("Model dosyası taşınamadı ({}): {}", file, e))?;
            tracing::info!("Embedding modeli indirildi: {}", dest.display());
        }

        Ok(dir)
    }

    /// Model dosyaları yerinde mi?
    pub fn files_ready() -> bool {
        let dir = Self::model_dir();
        MODEL_FILES.iter().all(|f| is_non_empty(&dir.join(f)))
    }

    /// Modeli belleğe yükler (ağır işlem — `spawn_blocking` içinde çağırın).
    pub fn load(dir: &Path) -> Result<Self, String> {
        let device = Device::Cpu;

        let config_path = dir.join("config.json");
        let config_raw = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Model yapılandırması okunamadı: {}", e))?;
        let config: BertConfig = serde_json::from_str(&config_raw)
            .map_err(|e| format!("Model yapılandırması geçersiz: {}", e))?;
        let hidden_size = config.hidden_size;

        let tokenizer_path = dir.join("tokenizer.json");
        let mut tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("Tokenizer açılamadı: {}", e))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_TOKENS,
                strategy: TruncationStrategy::LongestFirst,
                stride: 0,
                direction: tokenizers::tokenizer::TruncationDirection::Right,
            }))
            .map_err(|e| format!("Tokenizer kırpma ayarlanamadı: {}", e))?;
        let (pad_id, pad_token) = match tokenizer.get_padding() {
            Some(padding) => (padding.pad_id, padding.pad_token.clone()),
            None => {
                if tokenizer.token_to_id("[PAD]").is_some() {
                    (tokenizer.token_to_id("[PAD]").unwrap_or(0), "[PAD]".to_string())
                } else if tokenizer.token_to_id("<pad>").is_some() {
                    (tokenizer.token_to_id("<pad>").unwrap_or(0), "<pad>".to_string())
                } else {
                    (0, "[UNK]".to_string())
                }
            }
        };
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            direction: tokenizers::tokenizer::PaddingDirection::Right,
            pad_to_multiple_of: None,
            pad_id,
            pad_type_id: 0,
            pad_token,
        }));

        let weights_path = dir.join("model.safetensors");
        let weights = std::fs::read(&weights_path)
            .map_err(|e| format!("Model ağırlıkları okunamadı: {}", e))?;
        let vb = VarBuilder::from_buffered_safetensors(weights, DType::F32, &device)
            .map_err(|e| format!("Model ağırlıkları çözümlenemedi: {}", e))?;
        let model = BertModel::load(vb, &config).map_err(|e| format!("Model yüklenemedi: {}", e))?;

        if hidden_size != EMBEDDING_DIM {
            tracing::warn!(
                "Model boyutu {} ancak beklenen {}; vektör arama sonuçları farklı olabilir.",
                hidden_size,
                EMBEDDING_DIM
            );
        }

        tracing::info!(
            "Embedding modeli yüklendi: {} ({} boyut, CPU)",
            MODEL_NAME,
            hidden_size
        );

        Ok(Self {
            tokenizer,
            model,
            device,
            dim: hidden_size,
        })
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Tek metni gömür.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let mut result = self.embed_batch(&[text.to_string()])?;
        result
            .pop()
            .ok_or_else(|| "Embedding üretilemedi.".to_string())
    }

    /// Metin grubunu gömür. Girdi BATCH_SIZE parçalara bölünür.
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(BATCH_SIZE) {
            out.extend(self.encode_chunk(chunk)?);
        }
        Ok(out)
    }

    fn encode_chunk(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let inputs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let encodings = self
            .tokenizer
            .encode_batch(inputs, true)
            .map_err(|e| format!("Metin token'lara ayrılamadı: {}", e))?;

        let batch = encodings.len();
        let seq = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);
        if seq == 0 {
            return Err("Tokenize edilecek metin bulunamadı.".to_string());
        }

        let mut ids: Vec<u32> = Vec::with_capacity(batch * seq);
        let mut mask: Vec<u32> = Vec::with_capacity(batch * seq);
        let mut type_ids: Vec<u32> = Vec::with_capacity(batch * seq);
        for encoding in &encodings {
            ids.extend_from_slice(encoding.get_ids());
            mask.extend_from_slice(encoding.get_attention_mask());
            type_ids.extend_from_slice(encoding.get_type_ids());
        }

        let input_ids = Tensor::from_vec(ids, (batch, seq), &self.device)
            .map_err(|e| format!("Girdi tensörü oluşturulamadı: {}", e))?;
        let attention_mask = Tensor::from_vec(mask, (batch, seq), &self.device)
            .map_err(|e| format!("Dikkat maskesi oluşturulamadı: {}", e))?;
        let token_type_ids = Tensor::from_vec(type_ids, (batch, seq), &self.device)
            .map_err(|e| format!("Token tipi tensörü oluşturulamadı: {}", e))?;

        let hidden = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))
            .map_err(|e| format!("Model çalıştırılamadı: {}", e))?;

        let flat: Vec<f32> = hidden
            .flatten_all()
            .map_err(|e| format!("Model çıktısı düzleştirilemedi: {}", e))?
            .to_vec1::<f32>()
            .map_err(|e| format!("Model çıktısı okunamadı: {}", e))?;

        let mask_flat: Vec<f32> = attention_mask
            .flatten_all()
            .map_err(|e| format!("Maske düzleştirilemedi: {}", e))?
            .to_vec1::<u32>()
            .map_err(|e| format!("Maske okunamadı: {}", e))?
            .into_iter()
            .map(|v| v as f32)
            .collect();

        let hidden_size = self.dim;
        let mut out = Vec::with_capacity(batch);

        for row in 0..batch {
            let mut pooled = vec![0.0f32; hidden_size];
            let mut count = 0.0f32;
            for position in 0..seq {
                let weight = mask_flat[row * seq + position];
                if weight == 0.0 {
                    continue;
                }
                count += weight;
                let offset = (row * seq + position) * hidden_size;
                for (slot, value) in pooled.iter_mut().enumerate() {
                    *value += flat[offset + slot] * weight;
                }
            }
            if count > 0.0 {
                let inv = 1.0 / count;
                for value in pooled.iter_mut() {
                    *value *= inv;
                }
            }

            let norm = pooled.iter().map(|v| v * v).sum::<f32>().sqrt() + NORM_EPSILON;
            for value in pooled.iter_mut() {
                *value /= norm;
            }
            out.push(pooled);
        }

        Ok(out)
    }
}

fn is_non_empty(path: &Path) -> bool {
    std::fs::metadata(path).map(|m| m.is_file() && m.len() > 0).unwrap_or(false)
}

/// Embedding modelinin çalışma durumu.
#[derive(Clone)]
pub enum Snapshot {
    Idle,
    Loading,
    Ready(std::sync::Arc<Embedder>),
    Failed(String),
}

/// Uygulama boyunca tek bir embedding örneğini taşır.
///
/// Model ilk istekte indirilir/yüklenir; diğer istekler aynı anda beklemede kalır.
pub struct EmbeddingHandle {
    runtime: std::sync::Mutex<Snapshot>,
}

impl Default for EmbeddingHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingHandle {
    pub fn new() -> Self {
        Self {
            runtime: std::sync::Mutex::new(Snapshot::Idle),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.runtime.lock().unwrap().clone()
    }

    /// Yükleme görevini üstlenmek isteyen çağıran true döndürür.
    pub fn claim_loading(&self) -> bool {
        let mut guard = self.runtime.lock().unwrap();
        match *guard {
            Snapshot::Idle | Snapshot::Failed(_) => {
                *guard = Snapshot::Loading;
                true
            }
            _ => false,
        }
    }

    pub fn finish(&self, result: Result<std::sync::Arc<Embedder>, String>) {
        let mut guard = self.runtime.lock().unwrap();
        *guard = match result {
            Ok(embedder) => Snapshot::Ready(embedder),
            Err(message) => Snapshot::Failed(message),
        };
    }

    /// Bir sonraki denemeyi mümkün kılar (hata durumundan kurtulmak için).
    pub fn reset(&self) {
        let mut guard = self.runtime.lock().unwrap();
        if matches!(*guard, Snapshot::Failed(_)) {
            *guard = Snapshot::Idle;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_dir_is_inside_cache() {
        let dir = Embedder::model_dir();
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        assert_eq!(name, MODEL_NAME);
        assert!(dir.ends_with(Path::new("kukla").join("models").join(MODEL_NAME)));
    }

    #[test]
    fn embedding_dim_is_384() {
        assert_eq!(EMBEDDING_DIM, 384);
    }
}
