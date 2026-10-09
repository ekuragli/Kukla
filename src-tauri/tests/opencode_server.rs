use kukla_lib::core::opencode_server::{
    candidate_urls, endpoint_for_port, is_managed, resolved_url, spawn_server_with, PORTS,
};

#[test]
fn port_scan_order_is_deterministic() {
    assert_eq!(PORTS[0], 4096);
    assert_eq!(endpoint_for_port(4096), "http://127.0.0.1:4096");
    assert_eq!(endpoint_for_port(4100), "http://127.0.0.1:4100");
}

#[test]
fn candidate_urls_env_first_then_ports_unique() {
    let urls = candidate_urls(Some("http://127.0.0.1:5000/"));
    assert_eq!(urls.first().map(String::as_str), Some("http://127.0.0.1:5000"));
    for url in &urls {
        assert_eq!(urls.iter().filter(|other| other == &url).count(), 1);
    }
    // Tüm portlar temsil edilir.
    for port in PORTS {
        assert!(urls.contains(&endpoint_for_port(port)));
    }
}

#[test]
fn candidate_urls_without_env_matches_port_list() {
    let urls = candidate_urls(None);
    assert!(urls.contains(&"http://127.0.0.1:4096".to_string()));
    assert!(urls.len() >= PORTS.len());
}

#[test]
fn wait_healthy_accepts_when_probe_says_yes() {
    // Gerçek ağ yok: sağlık bekleme yardımcısını sahte sondajla doğrula.
    let health = kukla_lib::core::opencode_server::wait_healthy_with(
        "http://127.0.0.1:1",
        |_| async { true },
    );
    let health = tokio::runtime::Runtime::new().unwrap().block_on(health);
    assert!(health);
}

#[test]
fn spawn_with_missing_binary_reports_install_hint() {
    let error = spawn_server_with("kukla-bulunamayan-binary", 4096).unwrap_err();
    assert!(error.contains("opencode başlatılamadı"));
    assert!(error.contains("PATH"));
    assert!(error.contains("opencode.ai"));
}

#[test]
fn resolved_url_falls_back_to_default_port() {
    if std::env::var("KUKLA_OPENCODE_URL").is_err() && resolved_url().starts_with("http://127.0.0.1:") {
        assert_eq!(resolved_url(), endpoint_for_port(PORTS[0]));
    }
}

#[test]
fn shutdown_never_kills_a_server_it_did_not_start() {
    // Bayrak kapalıyken shutdown hiçbir şey yapmaz ve yönetici durumu kapalı kalır.
    kukla_lib::core::opencode_server::shutdown_server();
    assert!(!is_managed());
}
