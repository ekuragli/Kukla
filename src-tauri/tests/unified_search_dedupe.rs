use chrono::Utc;
use kukla_lib::commands::unified_search::{apply_source_filter, dedupe_results};
use kukla_lib::models::{Decision, DecisionSource, SearchResult};

fn result(id: i64, percent: u8, online: Option<&str>) -> SearchResult {
    SearchResult {
        decision: Decision {
            id,
            case_id: 0,
            source: DecisionSource::Yargitay,
            esas_no: Some(format!("E/{}", id)),
            karar_no: Some(format!("K/{}", id)),
            karar_tarihi: None,
            daire: None,
            summary: None,
            ratio_decidendi: None,
            similarity_score: None,
            created_at: Utc::now(),
        },
        similarity_percent: percent,
        snippet: String::new(),
        online_source: online.map(|s| s.to_string()),
    }
}

#[test]
fn dedupe_removes_duplicates_and_orders_online_first() {
    let mut results = vec![
        result(1, 10, None),
        result(1, 90, Some("yargitay")),
        result(2, 50, Some("bedesten")),
        result(3, 99, None),
    ];

    dedupe_results(&mut results);

    assert_eq!(results.len(), 3);
    // Çevrimiçi kaynaklar önce (Yargıtay > Bedesten), yerel sonuçlar en sonda.
    assert_eq!(results[0].online_source.as_deref(), Some("yargitay"));
    assert_eq!(results[0].decision.id, 1);
    assert_eq!(results[1].online_source.as_deref(), Some("bedesten"));
    assert_eq!(results[2].online_source.as_deref(), None);
    assert_eq!(results[2].decision.id, 3);
}

#[test]
fn dedupe_falls_back_to_reference_key_when_id_missing() {
    let mut results = vec![
        result(0, 40, Some("yargitay")),
        result(0, 80, Some("bedesten")),
    ];
    // id'siz sonuçlar aynı esas/karar/daire anahtarını paylaşır.
    for r in results.iter_mut() {
        r.decision.esas_no = Some("2026/1".to_string());
        r.decision.karar_no = Some("500".to_string());
        r.decision.daire = Some("9. Hukuk".to_string());
    }

    dedupe_results(&mut results);

    assert_eq!(results.len(), 1);
}

#[test]
fn source_filter_selects_online_or_local() {
    let mut results = vec![result(1, 10, None), result(2, 50, Some("yargitay"))];

    apply_source_filter(&mut results, Some("online"));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].decision.id, 2);

    let mut results = vec![result(1, 10, None), result(2, 50, Some("yargitay"))];
    apply_source_filter(&mut results, Some("local"));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].decision.id, 1);

    let mut results = vec![result(1, 10, None), result(2, 50, Some("yargitay"))];
    apply_source_filter(&mut results, None);
    assert_eq!(results.len(), 2);
}
