use kukla_lib::core::yargitay::client::{map_daire_code, YargitayClient};

#[test]
fn map_daire_code_recognizes_hukuk_chamber() {
    let (hukuk, ceza, kurul) = map_daire_code("4. Hukuk Dairesi");
    assert_eq!(hukuk.as_deref(), Some("4. Hukuk Dairesi"));
    assert!(ceza.is_none());
    assert!(kurul.is_none());
}

#[test]
fn map_daire_code_recognizes_h_code() {
    let (hukuk, _, _) = map_daire_code("H11");
    assert_eq!(hukuk.as_deref(), Some("11. Hukuk Dairesi"));
}

#[tokio::test]
#[ignore = "requires network access to karararama.yargitay.gov.tr"]
async fn live_yargitay_search_returns_results() {
    let client = YargitayClient::new();
    let result = client
        .search_decisions(
            "kira bedelinin uyarlanması",
            None,
            None,
            None,
            None,
            None,
            1,
            5,
            1,
        )
        .await
        .expect("live search should succeed");

    assert!(!result.decisions.is_empty(), "expected at least one decision");
    assert!(result.total_records > 0);
}