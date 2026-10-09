use kukla_lib::db::sqlite::MetadataDb;

fn memory_db() -> MetadataDb {
    MetadataDb::connect_memory().unwrap()
}

#[test]
fn creates_session_and_appends_messages() {
    let db = memory_db();
    let session_id = db.create_chat_session("Kira uyarlma", false).unwrap();

    let first = db
        .append_chat_message(session_id, "user", "Kira uyarlmada içtihat nedir?", None)
        .unwrap();
    let second = db
        .append_chat_message(
            session_id,
            "assistant",
            "Yargıtay'a göre ...",
            Some(r#"[{"label":"Esas: 2026/100","kind":"local"}]"#),
        )
        .unwrap();
    assert!(second > first);

    let messages = db.list_chat_messages(session_id).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
    assert!(messages[1].sources_json.is_some());

    let sessions = db.list_chat_sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "Kira uyarlma");
    assert_eq!(sessions[0].message_count, 2);
    assert!(!sessions[0].allow_external);
    assert!(sessions[0].created_at.timestamp() > 0);
    assert!(sessions[0].updated_at.is_some());
}

#[test]
fn external_flag_roundtrip() {
    let db = memory_db();
    let session_id = db.create_chat_session("Oturum", false).unwrap();
    db.set_chat_session_external(session_id, true).unwrap();

    let session = db.get_chat_session(session_id).unwrap().unwrap();
    assert!(session.allow_external);
}

#[test]
fn opencode_session_and_model_stored() {
    let db = memory_db();
    let session_id = db.create_chat_session("Oturum", false).unwrap();

    db.set_chat_session_opencode(session_id, Some("opencode/space-bunny-free"), Some("ses_123"))
        .unwrap();

    let session = db.get_chat_session(session_id).unwrap().unwrap();
    assert_eq!(session.opencode_session_id.as_deref(), Some("ses_123"));
    assert_eq!(session.model.as_deref(), Some("opencode/space-bunny-free"));

    db.set_chat_session_title(session_id, "Yeni başlık").unwrap();
    let session = db.get_chat_session(session_id).unwrap().unwrap();
    assert_eq!(session.title, "Yeni başlık");
}

#[test]
fn delete_session_removes_messages() {
    let db = memory_db();
    let session_id = db.create_chat_session("Silinecek", false).unwrap();
    db.append_chat_message(session_id, "user", "Merhaba", None)
        .unwrap();

    assert!(db.delete_chat_session(session_id).unwrap());
    assert!(db.list_chat_messages(session_id).unwrap().is_empty());
    assert!(db.list_chat_sessions().unwrap().is_empty());
    // Olmayan oturumu silmek false döner.
    assert!(!db.delete_chat_session(session_id).unwrap());
}

#[test]
fn missing_session_returns_none() {
    let db = memory_db();
    assert!(db.get_chat_session(999).unwrap().is_none());
    assert!(db.list_chat_messages(999).unwrap().is_empty());
}
