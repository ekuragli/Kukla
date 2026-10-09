use kukla_lib::commands::auth_guard::{
    locked_session_message, session_timed_out, validate_password_strength,
};
use std::time::{Duration, Instant};

#[test]
fn locked_session_message_when_not_authenticated() {
    let msg = locked_session_message(false).expect("mesaj olmalı");
    assert!(msg.contains("Oturum kilitli"));
}

#[test]
fn locked_session_message_none_when_authenticated() {
    assert!(locked_session_message(true).is_none());
}

#[test]
fn session_not_timed_out_when_auto_lock_disabled() {
    let last = Instant::now() - Duration::from_secs(3600);
    assert!(!session_timed_out(last, 0));
}

#[test]
fn session_timed_out_after_inactivity() {
    let last = Instant::now() - Duration::from_secs(120);
    assert!(session_timed_out(last, 1));
}

#[test]
fn password_rejects_short() {
    assert!(validate_password_strength("Ab1!abc").is_err());
}

#[test]
fn password_rejects_missing_categories() {
    assert!(validate_password_strength("shortpassword123").is_err());
    assert!(validate_password_strength("ALLUPPERCASE123!").is_err());
    assert!(validate_password_strength("alllowercase123!").is_err());
    assert!(validate_password_strength("NoDigitsHereAAAA!").is_err());
    assert!(validate_password_strength("NoSpecialChars123").is_err());
}

#[test]
fn password_accepts_strong() {
    assert!(validate_password_strength("GucluSifre123!").is_ok());
    assert!(validate_password_strength("Abcdef123456!").is_ok());
}