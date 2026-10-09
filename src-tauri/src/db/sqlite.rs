use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

use crate::models::*;

pub struct MetadataDb {
    conn: Mutex<Connection>,
}

impl MetadataDb {
    pub fn connect(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Veritabanı dizini oluşturulamadı")?;
        }

        let conn = Connection::open(path).context("Failed to open SQLite database")?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMA")?;

        let db = MetadataDb {
            conn: Mutex::new(conn),
        };
        db.initialize_tables()?;
        Ok(db)
    }

    pub fn connect_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Bellek içi veritabanı açılamadı")?;

        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMA")?;

        let db = MetadataDb {
            conn: Mutex::new(conn),
        };
        db.initialize_tables()?;
        Ok(db)
    }

    pub fn checkpoint(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .context("WAL checkpoint başarısız")?;
        Ok(())
    }

    fn initialize_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                settings_json TEXT NOT NULL,
                password_hash TEXT,
                updated_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS cases (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query_text TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT,
                pdf_path TEXT,
                pdf_hash TEXT,
                status TEXT DEFAULT 'active'
            );

            CREATE TABLE IF NOT EXISTS decisions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                case_id INTEGER,
                source TEXT NOT NULL,
                esas_no TEXT,
                karar_no TEXT,
                karar_tarihi TEXT,
                daire TEXT,
                summary TEXT,
                ratio_decidendi TEXT,
                full_text_path TEXT,
                similarity_score REAL,
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (case_id) REFERENCES cases(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS search_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                case_id INTEGER,
                query_text TEXT NOT NULL,
                result_count INTEGER DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (case_id) REFERENCES cases(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS audit_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                event_details TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS llm_summaries (
                decision_id INTEGER PRIMARY KEY,
                context_hash TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS chat_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                opencode_session_id TEXT,
                model TEXT,
                allow_external INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS chat_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                sources_json TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS ai_secrets (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                web_search_api_key TEXT,
                cloud_api_key TEXT,
                updated_at TEXT DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_cases_status ON cases(status);
            CREATE INDEX IF NOT EXISTS idx_cases_created_at ON cases(created_at);
            CREATE INDEX IF NOT EXISTS idx_decisions_case_id ON decisions(case_id);
            CREATE INDEX IF NOT EXISTS idx_decisions_source ON decisions(source);
            CREATE INDEX IF NOT EXISTS idx_decisions_similarity ON decisions(similarity_score);
            CREATE INDEX IF NOT EXISTS idx_audit_logs_event ON audit_logs(event_type);
            CREATE INDEX IF NOT EXISTS idx_chat_sessions_updated_at ON chat_sessions(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, id);
            ",
        )
        .context("Failed to initialize database tables")?;

        Ok(())
    }

    pub fn get_settings(&self) -> Result<AppSettings> {
        let conn = self.conn.lock().unwrap();
        let result: Result<String, _> = conn.query_row(
            "SELECT settings_json FROM app_settings WHERE id = 1",
            [],
            |row| row.get(0),
        );

        match result {
            Ok(json_str) => {
                let settings: AppSettings =
                    serde_json::from_str(&json_str).unwrap_or_default();
                Ok(settings)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let settings = AppSettings::default();
                let json_str = serde_json::to_string(&settings)?;
                conn.execute(
                    "INSERT INTO app_settings (id, settings_json) VALUES (1, ?1)",
                    params![json_str],
                )?;
                Ok(settings)
            }
            Err(e) => Err(e).context("Failed to read settings"),
        }
    }

    pub fn update_settings(&self, settings: &AppSettings) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let json_str = serde_json::to_string(settings)?;
        conn.execute(
            "INSERT INTO app_settings (id, settings_json, updated_at) VALUES (1, ?1, datetime('now'))
             ON CONFLICT(id) DO UPDATE SET settings_json = ?1, updated_at = datetime('now')",
            params![json_str],
        )?;
        Ok(())
    }

    /// Aynı karar + aynı bağlam için önbelleğe alınmış YZ özetini okur.
    pub fn get_llm_summary(&self, decision_id: i64, context_hash: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result: Result<String, _> = conn.query_row(
            "SELECT payload FROM llm_summaries WHERE decision_id = ?1 AND context_hash = ?2",
            params![decision_id, context_hash],
            |row| row.get(0),
        );
        match result {
            Ok(payload) => Ok(Some(payload)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e).context("Failed to read llm summary cache"),
        }
    }

    /// YZ özetini önbelleğe yazar (aynı karar tekrar özetlenirse anında döner).
    pub fn set_llm_summary(&self, decision_id: i64, context_hash: &str, payload: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO llm_summaries (decision_id, context_hash, payload) VALUES (?1, ?2, ?3)
             ON CONFLICT(decision_id) DO UPDATE SET context_hash = ?2, payload = ?3, created_at = datetime('now')",
            params![decision_id, context_hash, payload],
        )?;
        Ok(())
    }

    pub fn set_password_hash(&self, hash: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_settings (id, settings_json, password_hash) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET password_hash = ?2",
            params![serde_json::to_string(&AppSettings::default())?, hash],
        )?;
        Ok(())
    }

    pub fn get_password_hash(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result: Result<String, _> = conn.query_row(
            "SELECT password_hash FROM app_settings WHERE id = 1",
            [],
            |row| row.get(0),
        );
        match result {
            Ok(hash) => Ok(Some(hash)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e).context("Failed to read password hash"),
        }
    }

    pub fn insert_case(&self, new_case: &NewCase) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO cases (query_text, pdf_path, pdf_hash) VALUES (?1, ?2, ?3)",
            params![new_case.query_text, new_case.pdf_path, new_case.pdf_hash],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn insert_decision(&self, decision: &NewDecision) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let tarih_str = decision
            .karar_tarihi
            .map(|d| d.format("%Y-%m-%d").to_string());
        conn.execute(
            "INSERT INTO decisions (case_id, source, esas_no, karar_no, karar_tarihi, daire, summary, ratio_decidendi, similarity_score)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                decision.case_id,
                decision.source.as_str(),
                decision.esas_no,
                decision.karar_no,
                tarih_str,
                decision.daire,
                decision.summary,
                decision.ratio_decidendi,
                decision.similarity_score,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_case(&self, case_id: i64) -> Result<Option<Case>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, query_text, created_at, updated_at, pdf_path, pdf_hash, status FROM cases WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map(params![case_id], |row| {
            let created: String = row.get(2)?;
            let updated: Option<String> = row.get(3)?;
            Ok(Case {
                id: row.get(0)?,
                query_text: row.get(1)?,
                created_at: DateTime::parse_from_rfc3339(&created)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: updated
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.with_timezone(&Utc)),
                pdf_path: row.get(4)?,
                pdf_hash: row.get(5)?,
                status: row.get(6)?,
            })
        })?;

        match rows.next() {
            Some(Ok(case)) => Ok(Some(case)),
            Some(Err(e)) => Err(e).context("Failed to read case"),
            None => Ok(None),
        }
    }

    pub fn get_decisions_by_case(&self, case_id: i64) -> Result<Vec<Decision>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, case_id, source, esas_no, karar_no, karar_tarihi, daire, summary, ratio_decidendi, similarity_score, created_at
             FROM decisions WHERE case_id = ?1 ORDER BY similarity_score DESC",
        )?;

        let rows = stmt.query_map(params![case_id], |row| {
            let source_str: String = row.get(2)?;
            let tarih_str: Option<String> = row.get(5)?;
            let created: String = row.get(10)?;

            Ok(Decision {
                id: row.get(0)?,
                case_id: row.get(1)?,
                source: source_str.parse().unwrap_or(DecisionSource::Yargitay),
                esas_no: row.get(3)?,
                karar_no: row.get(4)?,
                karar_tarihi: tarih_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                daire: row.get(6)?,
                summary: row.get(7)?,
                ratio_decidendi: row.get(8)?,
                similarity_score: row.get(9)?,
                created_at: DateTime::parse_from_rfc3339(&created)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut decisions = Vec::new();
        for row in rows {
            decisions.push(row?);
        }
        Ok(decisions)
    }

    pub fn get_search_history(&self, limit: u32) -> Result<Vec<SearchHistory>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, case_id, query_text, result_count, created_at
             FROM search_history ORDER BY created_at DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            let created: String = row.get(4)?;
            Ok(SearchHistory {
                id: row.get(0)?,
                case_id: row.get(1)?,
                query_text: row.get(2)?,
                result_count: row.get(3)?,
                created_at: DateTime::parse_from_rfc3339(&created)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }
        Ok(history)
    }

    pub fn insert_search_history(&self, case_id: i64, query: &str, result_count: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO search_history (case_id, query_text, result_count) VALUES (?1, ?2, ?3)",
            params![case_id, query, result_count],
        )?;
        Ok(())
    }

    pub fn clear_search_history(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM search_history", [])?;
        Ok(deleted as u64)
    }

    pub fn insert_audit_log(&self, event_type: &str, details: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO audit_logs (event_type, event_details) VALUES (?1, ?2)",
            params![event_type, details],
        )?;
        Ok(())
    }

    pub fn delete_case(&self, case_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM cases WHERE id = ?1", params![case_id])?;
        Ok(())
    }

    pub fn delete_personal_decisions(&self) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id FROM decisions WHERE source = 'user_uploaded'",
        )?;
        let ids: Vec<i64> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        conn.execute("DELETE FROM decisions WHERE source = 'user_uploaded'", [])?;
        Ok(ids)
    }

    pub fn get_decision(&self, decision_id: i64) -> Result<Option<Decision>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, case_id, source, esas_no, karar_no, karar_tarihi, daire, summary, ratio_decidendi, similarity_score, created_at
             FROM decisions WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map(params![decision_id], |row| {
            let source_str: String = row.get(2)?;
            let tarih_str: Option<String> = row.get(5)?;
            let created: String = row.get(10)?;

            Ok(Decision {
                id: row.get(0)?,
                case_id: row.get(1)?,
                source: source_str.parse().unwrap_or(DecisionSource::Yargitay),
                esas_no: row.get(3)?,
                karar_no: row.get(4)?,
                karar_tarihi: tarih_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                daire: row.get(6)?,
                summary: row.get(7)?,
                ratio_decidendi: row.get(8)?,
                similarity_score: row.get(9)?,
                created_at: DateTime::parse_from_rfc3339(&created)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        match rows.next() {
            Some(Ok(decision)) => Ok(Some(decision)),
            Some(Err(e)) => Err(e).context("Failed to read decision"),
            None => Ok(None),
        }
    }

    pub fn list_decisions_by_source(&self, source: &str) -> Result<Vec<Decision>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, case_id, source, esas_no, karar_no, karar_tarihi, daire, summary, ratio_decidendi, similarity_score, created_at
             FROM decisions WHERE source = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map(params![source], |row| {
            let source_str: String = row.get(2)?;
            let tarih_str: Option<String> = row.get(5)?;
            let created: String = row.get(10)?;

            Ok(Decision {
                id: row.get(0)?,
                case_id: row.get(1)?,
                source: source_str.parse().unwrap_or(DecisionSource::Yargitay),
                esas_no: row.get(3)?,
                karar_no: row.get(4)?,
                karar_tarihi: tarih_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                daire: row.get(6)?,
                summary: row.get(7)?,
                ratio_decidendi: row.get(8)?,
                similarity_score: row.get(9)?,
                created_at: DateTime::parse_from_rfc3339(&created)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut decisions = Vec::new();
        for row in rows {
            decisions.push(row?);
        }
        Ok(decisions)
    }

    pub fn delete_decision(&self, decision_id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM decisions WHERE id = ?1", params![decision_id])?;
        Ok(affected > 0)
    }

    // ---- Chat ----

    pub fn create_chat_session(&self, title: &str, allow_external: bool) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO chat_sessions (title, allow_external) VALUES (?1, ?2)",
            params![title, allow_external as i32],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_chat_sessions(&self) -> Result<Vec<ChatSession>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.title, s.opencode_session_id, s.model, s.allow_external,
                    s.created_at, s.updated_at,
                    (SELECT COUNT(*) FROM chat_messages m WHERE m.session_id = s.id)
             FROM chat_sessions s
             ORDER BY s.updated_at DESC, s.id DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let created: String = row.get(5)?;
            let updated: Option<String> = row.get(6)?;
            Ok(ChatSession {
                id: row.get(0)?,
                title: row.get(1)?,
                opencode_session_id: row.get(2)?,
                model: row.get(3)?,
                allow_external: row.get::<_, i64>(4)? != 0,
                created_at: parse_sqlite_datetime(&created),
                updated_at: updated.as_deref().map(parse_sqlite_datetime),
                message_count: row.get(7)?,
            })
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        Ok(sessions)
    }

    pub fn get_chat_session(&self, session_id: i64) -> Result<Option<ChatSession>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, opencode_session_id, model, allow_external, created_at, updated_at
             FROM chat_sessions WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map(params![session_id], |row| {
            let created: String = row.get(5)?;
            let updated: Option<String> = row.get(6)?;
            Ok(ChatSession {
                id: row.get(0)?,
                title: row.get(1)?,
                opencode_session_id: row.get(2)?,
                model: row.get(3)?,
                allow_external: row.get::<_, i64>(4)? != 0,
                created_at: parse_sqlite_datetime(&created),
                updated_at: updated.as_deref().map(parse_sqlite_datetime),
                message_count: 0,
            })
        })?;

        match rows.next() {
            Some(Ok(session)) => Ok(Some(session)),
            Some(Err(e)) => Err(e).context("Failed to read chat session"),
            None => Ok(None),
        }
    }

    pub fn append_chat_message(
        &self,
        session_id: i64,
        role: &str,
        content: &str,
        sources_json: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO chat_messages (session_id, role, content, sources_json) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, role, content, sources_json],
        )?;
        let message_id = conn.last_insert_rowid();
        conn.execute(
            "UPDATE chat_sessions SET updated_at = datetime('now') WHERE id = ?1",
            params![session_id],
        )?;
        Ok(message_id)
    }

    pub fn list_chat_messages(&self, session_id: i64) -> Result<Vec<ChatMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, sources_json, created_at
             FROM chat_messages WHERE session_id = ?1 ORDER BY id ASC",
        )?;

        let rows = stmt.query_map(params![session_id], |row| {
            let created: String = row.get(5)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                sources_json: row.get(4)?,
                created_at: parse_sqlite_datetime(&created),
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    pub fn delete_chat_session(&self, session_id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM chat_messages WHERE session_id = ?1",
            params![session_id],
        )?;
        let affected = conn.execute(
            "DELETE FROM chat_sessions WHERE id = ?1",
            params![session_id],
        )?;
        Ok(affected > 0)
    }

    /// Oturumun opencode kimliğini/modelini günceller (çok turlu süreklilik için).
    pub fn set_chat_session_opencode(
        &self,
        session_id: i64,
        model: Option<&str>,
        opencode_session_id: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE chat_sessions SET model = ?2, opencode_session_id = ?3, updated_at = datetime('now')
             WHERE id = ?1",
            params![session_id, model, opencode_session_id],
        )?;
        Ok(())
    }

    pub fn set_chat_session_title(&self, session_id: i64, title: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE chat_sessions SET title = ?2 WHERE id = ?1",
            params![session_id, title],
        )?;
        Ok(())
    }

    pub fn set_chat_session_external(&self, session_id: i64, allow_external: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE chat_sessions SET allow_external = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![session_id, allow_external as i32],
        )?;
        Ok(())
    }

    // ---- Gizli YZ anahtarları (API anahtarları; şifreli veritabanında) ----

    pub fn get_ai_secrets(&self) -> Result<AiSecrets> {
        let conn = self.conn.lock().unwrap();
        let result: Result<(Option<String>, Option<String>), _> = conn.query_row(
            "SELECT web_search_api_key, cloud_api_key FROM ai_secrets WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        match result {
            Ok((web_search_api_key, cloud_api_key)) => Ok(AiSecrets {
                web_search_api_key,
                cloud_api_key,
            }),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(AiSecrets::default()),
            Err(e) => Err(e).context("Failed to read ai secrets"),
        }
    }

    /// Web arama anahtarını yazar; `None`/boş değer anahtarı siler.
    pub fn set_web_search_api_key(&self, key: Option<&str>) -> Result<()> {
        let value = key.map(str::trim).filter(|value| !value.is_empty());
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO ai_secrets (id, web_search_api_key) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET web_search_api_key = ?1, updated_at = datetime('now')",
            params![value],
        )?;
        Ok(())
    }

    /// Bulut sağlayıcı anahtarını yazar; `None`/boş değer anahtarı siler.
    pub fn set_cloud_api_key(&self, key: Option<&str>) -> Result<()> {
        let value = key.map(str::trim).filter(|value| !value.is_empty());
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO ai_secrets (id, cloud_api_key) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET cloud_api_key = ?1, updated_at = datetime('now')",
            params![value],
        )?;
        Ok(())
    }
}

/// SQLite'in ürettisi hem `YYYY-MM-DD HH:MM:SS` hem RFC3339 biçimini kapsar.
fn parse_sqlite_datetime(value: &str) -> DateTime<Utc> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return parsed.with_timezone(&Utc);
    }
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc))
        .unwrap_or_else(|_| Utc::now())
}


