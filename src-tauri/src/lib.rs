pub mod commands;
pub mod core;
pub mod db;
pub mod models;
pub mod utils;

use std::path::PathBuf;
use std::sync::RwLock;
use std::time::Instant;
use tauri::Manager;
use zeroize::Zeroizing;

use core::audit::AuditLogger;
use core::bedesten::client::BedestenClient;
use core::yargitay::client::YargitayClient;
use core::encryption::credentials::AuthCredentials;
use core::encryption::vault;


use db::vector_store::VectorDb;
use db::sqlite::MetadataDb;

pub const APP_NAME: &str = "Kukla";

pub struct AppState {
    pub data_dir: PathBuf,
    pub db: RwLock<MetadataDb>,
    pub vector_db: RwLock<VectorDb>,
    pub embedding: core::embedding::EmbeddingHandle,
    pub db_key: std::sync::Mutex<Option<Zeroizing<[u8; 32]>>>,
    pub audit: std::sync::Arc<AuditLogger>,
    pub bedesten_client: std::sync::Arc<BedestenClient>,
    pub yargitay_client: std::sync::Arc<YargitayClient>,
    pub app_handle: std::sync::Mutex<Option<tauri::AppHandle>>,
    pub password_hash: std::sync::Mutex<Option<Zeroizing<String>>>,
    pub is_authenticated: std::sync::Mutex<bool>,
    pub last_activity: std::sync::Mutex<Instant>,
    pub login_attempts: std::sync::Mutex<u32>,
    pub lockout_until: std::sync::Mutex<Option<Instant>>,
}

impl AppState {
    fn activate_session(&self, password: &str, creds: &AuthCredentials) -> Result<(), String> {
        let salt = creds
            .salt_bytes()
            .map_err(|e| format!("Şifreleme tuzu okunamadı: {}", e))?;
        let key = vault::derive_storage_key(password, &salt)
            .map_err(|e| format!("Depolama anahtarı türetilemedi: {}", e))?;

        let (db, vector_db) = core::session::open_data_stores(&self.data_dir, &key)?;

        self.audit.set_encryption_key(key);
        *self.db_key.lock().unwrap() = Some(Zeroizing::new(key));
        *self.db.write().map_err(|_| "Veritabanı kilitli.".to_string())? = db;
        *self
            .vector_db
            .write()
            .map_err(|_| "Vector store kilitli.".to_string())? = vector_db;

        Ok(())
    }

    /// Veri dosyalarını verilen anahtarla şifreler ve dosya tutamaçlarını kapatır.
    pub fn seal_with_key(&self, key: &[u8; 32]) -> Result<(), String> {
        core::session::seal_data_stores(&self.db, &self.vector_db, &self.data_dir, key)
    }

    /// Şifre değişiminde verileri yeni anahtarla yeniden şifreler ve oturumu sürdürür.
    ///
    /// Şifre değiştirilip yalnızca bellek içi anahtar güncellenirse, diskteki
    /// şifreli kopyalar eski anahtarla kalır ve yeni şifreyle açılamaz.
    pub fn rekey_session(&self, new_key: &[u8; 32]) -> Result<(), String> {
        core::session::rekey_data_stores(&self.db, &self.vector_db, &self.data_dir, new_key)?;
        self.audit.set_encryption_key(*new_key);
        *self.db_key.lock().unwrap() = Some(Zeroizing::new(*new_key));

        Ok(())
    }
}

fn get_app_data_dir() -> PathBuf {
    let base = dirs_next::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_NAME)
}



#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_max_level(tracing::Level::INFO)
        .init();

    let data_dir = get_app_data_dir();
    std::fs::create_dir_all(&data_dir).expect("Failed to create app data directory");

    let _ = core::encryption::credentials::migrate_auth_from_db(&data_dir);

    let log_path = data_dir.join("audit.log");
    let audit = AuditLogger::new(Some(log_path));

    audit.log_app_open();
    audit.cleanup_old_logs(90);

    let db = MetadataDb::connect_memory().expect("Failed to initialize in-memory database");
    let vector_db = VectorDb::new_in_memory();

    let state = AppState {
        data_dir,
        db: RwLock::new(db),
        vector_db: RwLock::new(vector_db),
        embedding: core::embedding::EmbeddingHandle::new(),
        db_key: std::sync::Mutex::new(None),
        audit: std::sync::Arc::new(audit),
        bedesten_client: std::sync::Arc::new(BedestenClient::new()),
        yargitay_client: std::sync::Arc::new(YargitayClient::new()),
        app_handle: std::sync::Mutex::new(None),
        password_hash: std::sync::Mutex::new(None),
        is_authenticated: std::sync::Mutex::new(false),
        last_activity: std::sync::Mutex::new(Instant::now()),
        login_attempts: std::sync::Mutex::new(0),
        lockout_until: std::sync::Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            let handle = app.handle().clone();
            let state_ref: tauri::State<'_, AppState> = app.state();
            *state_ref.app_handle.lock().unwrap() = Some(handle.clone());

            if let Ok(Some(creds)) = AuthCredentials::load(&state_ref.data_dir) {
                *state_ref.password_hash.lock().unwrap() =
                    Some(Zeroizing::new(creds.password_hash.clone()));
            }

            // Pencere kapatıldığında oturum açıksa verileri şifrele; aksi halde
            // düz metin veritabanı ve vektör deposu diskte kalır.
            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        let state = handle.state::<AppState>();
                        let authenticated = *state.is_authenticated.lock().unwrap();
                        if authenticated {
                            if let Err(e) = commands::auth_guard::seal_session_data(&state) {
                                tracing::error!(
                                    "Kapatma sırasında veriler şifrelenemedi: {}",
                                    e
                                );
                            }
                        }
                        // Yalnızca uygulamanın başlattığı opencode sunucusunu durdur.
                        crate::core::opencode_server::shutdown_server();
                    }
                });
            }

            // YZ sunucusunu arka planda hazırla (ilk sohbet mesajı beklemesin).
            tauri::async_runtime::spawn(async {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if let Err(error) = crate::core::opencode_server::ensure_server().await {
                    tracing::debug!("opencode sunucusu ısıtması atlandı: {}", error);
                }
            });

            Ok(())
        })
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::search::search_case,
            commands::unified_search::search_unified,
            commands::summarize::summarize_decision,
            commands::app_paths::get_app_paths,
            commands::petition::get_petition_templates,
            commands::petition::generate_petition_draft,
            commands::petition::export_petition_draft,
            commands::chat::chat_new_session,
            commands::chat::chat_sessions,
            commands::chat::chat_history,
            commands::chat::chat_send,
            commands::chat::chat_delete_session,
            commands::chat::chat_set_external,
            commands::websearch::get_web_search_status,
            commands::websearch::set_web_search_key,
            commands::websearch::web_search,
            commands::cloud::get_cloud_status,
            commands::cloud::set_cloud_config,
            commands::cloud::set_cloud_key,
            commands::decision_index::clear_online_index_command,
            commands::upload_pdf::upload_and_index_pdf,
            commands::export::export_report,
            commands::export::export_section,
            commands::convert::convert_file,
            commands::bedesten_search::search_bedesten,
            commands::bedesten_search::get_bedesten_document,
            commands::yargitay_search::get_yargitay_document,
            commands::open_link::open_decision_in_browser,
            commands::search_history::get_search_history,
            commands::search_history::clear_search_history,
            commands::personal_archive::add_personal_decision,
            commands::personal_archive::add_personal_decisions_bulk,
            commands::personal_archive::list_personal_decisions,
            commands::personal_archive::delete_personal_decision,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::opencode::get_ai_status,
            commands::opencode::abort_ai_request,
            commands::opencode::start_opencode_server,
            commands::embedding::prepare_embedding,
            commands::embedding::get_embedding_status,
            auth_unlock,
            auth_set_password,
            auth_check,
            auth_lock,
            auth_ping_activity,
            auth_check_session,
            get_lockout_remaining,
            log_audit_event,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn auth_unlock(
    state: tauri::State<'_, AppState>,
    password: String,
) -> Result<bool, String> {
    {
        let mut lockout = state.lockout_until.lock().unwrap();
        if lockout.is_none() {
            *lockout = commands::auth_guard::load_persisted_lockout(&state);
        }
        if let Some(until) = *lockout {
            let remaining = until.saturating_duration_since(Instant::now());
            if remaining > std::time::Duration::ZERO {
                let secs = remaining.as_secs();
                return Err(format!(
                    "Çok fazla başarısız deneme. Lütfen {} saniye bekleyin.",
                    secs
                ));
            }
        }
    }

    let pwd = Zeroizing::new(password);
    let stored_clone = {
        let hash = state.password_hash.lock().unwrap();
        hash.as_ref().map(|s| s.clone())
    };
    match stored_clone.as_ref() {
        Some(stored) => {
            let valid = core::encryption::EncryptionService::verify_password(&pwd, stored)
                .map_err(|e| format!("Şifre doğrulama hatası: {}", e))?;
            if valid {
                let creds = AuthCredentials::load(&state.data_dir)
                    .map_err(|e| format!("Kimlik bilgileri okunamadı: {}", e))?
                    .ok_or_else(|| "Kimlik bilgileri bulunamadı.".to_string())?;

                state.activate_session(&pwd, &creds)?;

                *state.login_attempts.lock().unwrap() = 0;
                *state.lockout_until.lock().unwrap() = None;
                commands::auth_guard::clear_persisted_lockout(&state);
                *state.is_authenticated.lock().unwrap() = true;
                commands::auth_guard::reset_activity(&state);
                state.audit.log_login_success();
                Ok(true)
            } else {
                let mut attempts = state.login_attempts.lock().unwrap();
                *attempts += 1;
                state.audit.log_login_fail(*attempts);
                if *attempts >= 5 {
                    let until = Instant::now() + std::time::Duration::from_secs(300);
                    *state.lockout_until.lock().unwrap() = Some(until);
                    commands::auth_guard::persist_lockout(&state, until);
                    drop(attempts);
                    return Err(
                        "5 başarısız deneme. Hesap 5 dakika süreyle kilitlenmiştir.".to_string(),
                    );
                }
                Ok(false)
            }
        }
        None => {
            commands::auth_guard::validate_password_strength(&pwd)
                .map_err(|e| format!("Zayıf şifre: {}", e))?;

            let hashed = Zeroizing::new(
                core::encryption::EncryptionService::hash_password(&pwd)
                    .map_err(|e| format!("Şifre oluşturma hatası: {}", e))?,
            );
            let creds = AuthCredentials::create_new(&hashed);
            creds
                .save(&state.data_dir)
                .map_err(|e| format!("Kimlik bilgileri kaydedilemedi: {}", e))?;

            state.activate_session(&pwd, &creds)?;

            *state.password_hash.lock().unwrap() = Some(hashed);
            *state.is_authenticated.lock().unwrap() = true;
            commands::auth_guard::reset_activity(&state);
            state.audit.log_login_success();
            Ok(true)
        }
    }
}

#[tauri::command]
async fn auth_set_password(
    state: tauri::State<'_, AppState>,
    current_password: String,
    new_password: String,
) -> Result<bool, String> {
    let has_password = state.password_hash.lock().unwrap().is_some();
    if has_password {
        commands::auth_guard::require_auth(&state)?;
    }

    commands::auth_guard::validate_password_strength(&new_password)?;

    let current = Zeroizing::new(current_password);
    let new = Zeroizing::new(new_password);

    let hash = state.password_hash.lock().unwrap();
    if let Some(stored) = hash.as_ref() {
        let valid = core::encryption::EncryptionService::verify_password(&current, stored)
            .map_err(|e| format!("Mevcut şifre doğrulama hatası: {}", e))?;
        if !valid {
            return Err("Mevcut şifre yanlış.".to_string());
        }
    }
    drop(hash);

    let new_hash = Zeroizing::new(
        core::encryption::EncryptionService::hash_password(&new)
            .map_err(|e| format!("Şifre oluşturma hatası: {}", e))?,
    );

    let loaded_creds = AuthCredentials::load(&state.data_dir)
        .map_err(|e| format!("Kimlik bilgileri okunamadı: {}", e))?;
    let mut creds = loaded_creds
        .clone()
        .unwrap_or_else(|| AuthCredentials::create_new(&new_hash));
    creds.password_hash = new_hash.to_string();

    if !*state.is_authenticated.lock().unwrap() {
        // Kilit ekranından şifre değiştirme: hash'i değiştirip diskteki şifreli
        // veriyi yeni anahtarla yeniden şifrelemezsek eski veriler açılamaz.
        if loaded_creds.is_none() {
            creds
                .save(&state.data_dir)
                .map_err(|e| format!("Kimlik bilgileri güncellenemedi: {}", e))?;
            *state.password_hash.lock().unwrap() = Some(new_hash);
            return Ok(true);
        }

        let salt = creds
            .salt_bytes()
            .map_err(|e| format!("Şifreleme tuzu okunamadı: {}", e))?;
        let old_key = vault::derive_storage_key(&current, &salt)
            .map_err(|e| format!("Depolama anahtarı türetilemedi: {}", e))?;
        let new_key = vault::derive_storage_key(&new, &salt)
            .map_err(|e| format!("Depolama anahtarı türetilemedi: {}", e))?;

        let (db, vector_db) =
            core::session::open_data_stores(&state.data_dir, &old_key)?;
        let db = RwLock::new(db);
        let vector_db = RwLock::new(vector_db);
        core::session::seal_data_stores(&db, &vector_db, &state.data_dir, &new_key)?;

        if let Err(e) = creds.save(&state.data_dir) {
            // Kimlik bilgileri kaydedilemezse anahtarı geri al.
            if let Ok((db, vector_db)) =
                core::session::open_data_stores(&state.data_dir, &new_key)
            {
                let db = RwLock::new(db);
                let vector_db = RwLock::new(vector_db);
                let _ = core::session::seal_data_stores(
                    &db,
                    &vector_db,
                    &state.data_dir,
                    &old_key,
                );
            }
            return Err(format!("Kimlik bilgileri güncellenemedi: {}", e));
        }

        *state.password_hash.lock().unwrap() = Some(new_hash);
        return Ok(true);
    }

    // Oturum açıkken şifre değiştiğinde diskteki veriler yeni anahtarla yeniden
    // şifrelenmeli; aksi halde kilit açıldığında eski veriler çözülemez.
    let salt = creds
        .salt_bytes()
        .map_err(|e| format!("Şifreleme tuzu okunamadı: {}", e))?;
    let new_key = vault::derive_storage_key(&new, &salt)
        .map_err(|e| format!("Depolama anahtarı türetilemedi: {}", e))?;

    state.rekey_session(&new_key)?;

    if let Err(e) = creds.save(&state.data_dir) {
        // Kimlik bilgileri kaydedilemezse anahtarı geri al (eski şifreyle açılabilir kalsın).
        if let Ok(rollback_key) = vault::derive_storage_key(&current, &salt) {
            let _ = state.rekey_session(&rollback_key);
        }
        return Err(format!("Kimlik bilgileri güncellenemedi: {}", e));
    }

    *state.password_hash.lock().unwrap() = Some(new_hash);

    Ok(true)
}

#[tauri::command]
async fn auth_check(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let hash = state.password_hash.lock().unwrap();
    Ok(hash.is_some())
}

#[tauri::command]
async fn auth_lock(state: tauri::State<'_, AppState>) -> Result<(), String> {
    commands::auth_guard::seal_session_data(&state)?;
    *state.is_authenticated.lock().unwrap() = false;
    state
        .audit
        .log(crate::models::AuditEventType::AppLock, Some("manual_lock"));
    Ok(())
}

#[tauri::command]
async fn auth_ping_activity(state: tauri::State<'_, AppState>) -> Result<(), String> {
    commands::auth_guard::touch_activity(&state)
}

#[tauri::command]
async fn auth_check_session(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    Ok(commands::auth_guard::check_session(&state).is_ok())
}

#[tauri::command]
async fn log_audit_event(
    state: tauri::State<'_, AppState>,
    event_type: String,
    details: Option<String>,
) -> Result<(), String> {
    commands::auth_guard::require_auth(&state)?;

    let event = match event_type.as_str() {
        "app_lock" => crate::models::AuditEventType::AppLock,
        "settings_change" => crate::models::AuditEventType::SettingsChange,
        _ => return Err(format!("Unknown audit event: {}", event_type)),
    };
    state.audit.log(event, details.as_deref());
    Ok(())
}

#[tauri::command]
async fn get_lockout_remaining(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    let lockout = state.lockout_until.lock().unwrap();
    match *lockout {
        Some(until) => {
            let remaining = until.saturating_duration_since(Instant::now());
            Ok(remaining.as_secs())
        }
        None => Ok(0),
    }
}