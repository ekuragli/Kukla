use kukla_lib::core::yargitay::client::YargitayClient;

#[tokio::test]
#[ignore = "requires network access to karararama.yargitay.gov.tr"]
async fn filtered_yargitay_search_uses_simple_search() {
    let client = YargitayClient::new();
    let result = client
        .search_decisions(
            "kira",
            Some("6. Hukuk Dairesi"),
            None,
            None,
            Some(2015),
            Some(2020),
            1,
            10,
            3,
        )
        .await
        .expect("filtered search should succeed without detailed API");

    assert!(
        !result.decisions.is_empty(),
        "expected at least one decision for filtered search"
    );
}