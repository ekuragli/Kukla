use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::State;

use crate::core::embedding::{Embedder, Snapshot, EMBEDDING_DIM, MODEL_NAME};
use crate::models::EmbeddingStatus;
use crate::AppState;

const WAIT_LIMIT: Duration = Duration::from_secs(300);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Modeli indirir ve yükler; arayüze güncel durumu döndürür.
#[tauri::command]
pub async fn prepare_embedding(state: State<'_, AppState>) -> Result<EmbeddingStatus, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    state.embedding.reset();
    ensure_embedding(&state).await?;
    Ok(embedding_status(&state))
}

/// Yalnızca durum sorgusu (indirme/yükleme yapmaz).
#[tauri::command]
pub async fn get_embedding_status(state: State<'_, AppState>) -> Result<EmbeddingStatus, String> {
    crate::commands::auth_guard::require_auth(&state)?;
    Ok(embedding_status(&state))
}

pub fn embedding_status(state: &AppState) -> EmbeddingStatus {
    let (name, detail) = match state.embedding.snapshot() {
        Snapshot::Ready(_) => ("ready".to_string(), None),
        Snapshot::Loading => ("loading".to_string(), None),
        Snapshot::Failed(message) => ("error".to_string(), Some(message)),
        Snapshot::Idle => {
            if Embedder::files_ready() {
                ("not_loaded".to_string(), None)
            } else {
                ("missing".to_string(), None)
            }
        }
    };

    EmbeddingStatus {
        state: name,
        dim: EMBEDDING_DIM,
        model: MODEL_NAME.to_string(),
        path: Embedder::model_dir().display().to_string(),
        detail,
    }
}

/// Embedding modelini hazır eder; gerekiyorsa indirip belleğe yükler.
///
/// Aynı anda gelen çağrılar tek bir yükleme etrafında toplanır.
pub async fn ensure_embedding(state: &AppState) -> Result<Arc<Embedder>, String> {
    let deadline = Instant::now() + WAIT_LIMIT;

    loop {
        match state.embedding.snapshot() {
            Snapshot::Ready(embedder) => return Ok(embedder),
            Snapshot::Failed(message) => return Err(message),
            Snapshot::Idle => {
                if state.embedding.claim_loading() {
                    break;
                }
            }
            Snapshot::Loading => {
                if Instant::now() >= deadline {
                    return Err(
                        "Embedding modeli yükleniyor ancak işlem zaman aşımına uğradı.".to_string(),
                    );
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }

    let result = load_embedding().await;
    let outcome = match &result {
        Ok(embedder) => Ok(embedder.clone()),
        Err(message) => Err(message.clone()),
    };
    state.embedding.finish(result);
    outcome
}

/// Arama sırasında kullanılır: model dosyaları indirilmemişse indirme yapmaz,
/// sadece hazır olanla çalışır.
pub async fn ensure_embedding_cached(
    state: &AppState,
) -> Result<Arc<Embedder>, String> {
    if !Embedder::files_ready() {
        return Err(
            "Embedding modeli henüz indirilmedi. Ayarlar > YZ bölümünden indirebilirsiniz."
                .to_string(),
        );
    }
    ensure_embedding(state).await
}

async fn load_embedding() -> Result<Arc<Embedder>, String> {
    if !Embedder::files_ready() {
        tracing::info!("Embedding modeli indiriliyor: {}", MODEL_NAME);
        Embedder::download_missing().await?;
    }

    let dir = Embedder::model_dir();
    let loaded = tokio::task::spawn_blocking(move || Embedder::load(&dir))
        .await
        .map_err(|e| format!("Embedding yükleme görevi başarısız: {}", e))?;
    loaded.map(Arc::new)
}
