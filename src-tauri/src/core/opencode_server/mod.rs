//! opencode sunucusunu uygulama yönetiminde çalıştırır.
//!
//! Kullanıcının terminalde `opencode serve` başlatması gerekmesin diye YZ isteği
//! gelmeden önce sunucu kontrol edilir; ulaşılamıyorsa gizli bir alt süreç
//! olarak başlatılır ve sağlıklı olana kadar beklenir. Uygulama kapanırken
//! yalnızca uygulamanın kendi başlattığı süreç sonlandırılır — kullanıcının
//! çalışan sunucusuna dokunulmaz.

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, RwLock};
use std::time::{Duration, Instant};

/// Kullanılan varsayılan portlar (sırayla denenir).
pub const PORTS: [u16; 4] = [4096, 4097, 4098, 4099];
pub const DEFAULT_HOST: &str = "127.0.0.1";
/// Sunucunun sağlıklı hale gelmesi için beklenen azami süre.
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(20);
/// Sağlık yoklama aralığı.
pub const HEALTH_INTERVAL: Duration = Duration::from_millis(400);

/// Uygulamanın başlattığı alt süreç (varsa).
static SPAWNED_CHILD: LazyLock<RwLock<Option<Child>>> = LazyLock::new(|| RwLock::new(None));
/// Sunucuyu uygulama mı başlattı? (Kullanıcının sunucusunu öldürmemek için.)
static SPAWNED_BY_US: AtomicBool = AtomicBool::new(false);
/// Çalışma zamanında bulunan sunucu adresi (port kaydırmasında güncellenir).
static RUNTIME_URL: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| RwLock::new(None));
/// Aynı anda tek başlatma denemesi (ılık/ilk istek yarışı).
static ENSURE_LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Sunucu adresini üretir.
pub fn endpoint_for_port(port: u16) -> String {
    format!("http://{}:{}", DEFAULT_HOST, port)
}

/// Denenecek adreslerin sırası: ortam değişkeni → çalışma zamanı adresi → portlar.
pub fn candidate_urls(env_url: Option<&str>) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(url) = env_url.map(str::trim).filter(|value| !value.is_empty()) {
        urls.push(url.trim_end_matches('/').to_string());
    }
    if let Some(runtime) = runtime_url() {
        if !urls.contains(&runtime) {
            urls.push(runtime);
        }
    }
    for port in PORTS {
        let url = endpoint_for_port(port);
        if !urls.contains(&url) {
            urls.push(url);
        }
    }
    urls
}

/// Çalışma zamanında bulunan sunucu adresi.
pub fn runtime_url() -> Option<String> {
    RUNTIME_URL.read().ok().and_then(|guard| guard.clone())
}

fn set_runtime_url(url: &str) {
    if let Ok(mut guard) = RUNTIME_URL.write() {
        *guard = Some(url.to_string());
    }
}

/// YZ isteklerinin gittiği adres (çalışma zamanı → ortam değişkeni → varsayılan).
pub fn resolved_url() -> String {
    runtime_url()
        .or_else(|| {
            std::env::var("KUKLA_OPENCODE_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| endpoint_for_port(PORTS[0]))
}

/// Sunucuyu uygulama mı yönetiyor?
pub fn is_managed() -> bool {
    SPAWNED_BY_US.load(Ordering::SeqCst)
}

/// Gizli alt süreç olarak opencode sunucusunu başlatır (testlerde ikili adı
/// enjekte edilebilir).
pub fn spawn_server_with(binary: &str, port: u16) -> Result<Child, String> {
    let mut command = Command::new(binary);
    command.args([
        "serve",
        "--port",
        &port.to_string(),
        "--hostname",
        DEFAULT_HOST,
    ]);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Konsol penceresi açılmasın.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            format!(
                "opencode başlatılamadı ({}). opencode kurulu mu ve PATH'te mi? https://opencode.ai",
                error
            )
        })
}

pub fn spawn_server(port: u16) -> Result<Child, String> {
    spawn_server_with("opencode", port)
}

/// Sunucu sağlıklı olana kadar bekler (sondaj enjekte edilebilir).
pub async fn wait_healthy_with<F, Fut>(url: &str, probe: F) -> bool
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let mut probe = probe;
    let start = Instant::now();
    while start.elapsed() < HEALTH_TIMEOUT {
        if probe(url.to_string()).await {
            return true;
        }
        tokio::time::sleep(HEALTH_INTERVAL).await;
    }
    false
}

/// Bir adresin sağlık ucunu sorgular.
pub async fn probe_health(url: &str) -> bool {
    let client = crate::core::opencode_client::OpencodeClient::new(url);
    matches!(client.health().await, Ok(true))
}

/// Sunucuyu gerekirse başlatır ve kullanılabilir adresi döner.
///
/// 1. Bilinen adreslerden biri sağlıklıysa onu kullan (kullanıcının kendi
///    sunucusu da olabilir — başlatma yapılmaz).
/// 2. Hiçbiri değilse portları sırayla dene: sağlıklıysa kullan, değilse
///    uygulama yönetiminde gizli sunucu başlat ve bekle.
/// 3. Başlatılan süreç kimliği saklanır; kapanışta öldürülür.
pub async fn ensure_server() -> Result<String, String> {
    let _guard = ENSURE_LOCK.lock().await;

    // Kilit kazanıldığında başka bir çağrı sunucuyu hazırlamış olabilir.
    if let Some(url) = runtime_url() {
        if probe_health(&url).await {
            return Ok(url);
        }
    }

    for url in candidate_urls(None) {
        if probe_health(&url).await {
            set_runtime_url(&url);
            return Ok(url);
        }
    }

    let mut last_error = "opencode sunucusu başlatılamadı.".to_string();
    for port in PORTS {
        let url = endpoint_for_port(port);
        let child = match spawn_server(port) {
            Ok(child) => child,
            Err(error) => {
                last_error = error;
                continue;
            }
        };

        if wait_healthy_with(&url, |url| async move { probe_health(&url).await }).await {
            if let Ok(mut guard) = SPAWNED_CHILD.write() {
                *guard = Some(child);
            }
            SPAWNED_BY_US.store(true, Ordering::SeqCst);
            set_runtime_url(&url);
            tracing::info!("opencode sunucusu uygulama tarafından başlatıldı: {}", url);
            return Ok(url);
        }

        // Bu portta sunucu açılmadı: süreci temizle, sonraki porta geç.
        let mut child = child;
        let _ = child.kill();
    }

    Err(format!(
        "{}. İnternet araması ve yerel arşiv çalışmaya devam eder.",
        last_error
    ))
}

/// Uygulama kapanırken yalnızca kendi başlattığı sunucuyu durdurur.
pub fn shutdown_server() {
    if !SPAWNED_BY_US.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Ok(mut guard) = SPAWNED_CHILD.write() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
            tracing::info!("opencode sunucusu durduruldu (uygulama başlatmıştı)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_format() {
        assert_eq!(endpoint_for_port(4096), "http://127.0.0.1:4096");
    }

    #[test]
    fn candidate_urls_prefer_env_and_avoid_duplicates() {
        let urls = candidate_urls(Some("http://localhost:5000/"));
        assert_eq!(urls[0], "http://localhost:5000");
        // Port listesindeki adresler de sırayla eklenir.
        assert!(urls.contains(&"http://127.0.0.1:4096".to_string()));
        assert!(urls.contains(&"http://127.0.0.1:4097".to_string()));
        assert_eq!(
            urls.iter().filter(|url| *url == "http://127.0.0.1:4096").count(),
            1
        );
    }

    #[tokio::test]
    async fn wait_healthy_succeeds_when_probe_accepts() {
        // 400ms aralık yerine hızlı dönüşlü sahte sondaj.
        let healthy = wait_healthy_with("http://x", |_| async { true }).await;
        assert!(healthy);
    }

    #[tokio::test]
    async fn spawn_missing_binary_reports_turkish_error() {
        let error = spawn_server_with("kukla-yok-binary-xyz", 4096).unwrap_err();
        assert!(error.contains("opencode başlatılamadı"));
        assert!(error.contains("PATH"));
    }

    #[test]
    fn shutdown_skips_when_not_spawned_by_app() {
        // Kullanıcının kendi sunucusu için bayrak kapalı olmalı.
        SPAWNED_BY_US.store(false, Ordering::SeqCst);
        shutdown_server();
        assert!(!is_managed());
    }

    #[test]
    fn resolved_url_defaults_to_first_port() {
        // Ortam değişkeni tanımsızsa ilk port kullanılır.
        if std::env::var("KUKLA_OPENCODE_URL").is_err() {
            assert_eq!(resolved_url(), endpoint_for_port(PORTS[0]));
        }
    }
}
