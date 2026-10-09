use kukla_lib::commands::bedesten_search::{
    decision_matches_year_range, parse_turkish_date,
};
use kukla_lib::models::DecisionSource;
use std::str::FromStr;

#[test]
fn decision_source_roundtrip() {
    assert_eq!(DecisionSource::from_str("yargitay").unwrap().as_str(), "yargitay");
    assert_eq!(DecisionSource::from_str("danistay").unwrap().as_str(), "danistay");
    assert_eq!(DecisionSource::from_str("bam").unwrap().as_str(), "bam");
    assert_eq!(DecisionSource::from_str("user_uploaded").unwrap().as_str(), "user_uploaded");
    assert!(DecisionSource::from_str("unknown").is_err());
    assert_eq!(DecisionSource::Yargitay.as_str(), "yargitay");
    assert_eq!(DecisionSource::Danistay.as_str(), "danistay");
}

#[test]
fn parses_turkish_date() {
    assert_eq!(
        parse_turkish_date("15.03.2022"),
        Some(chrono::NaiveDate::from_ymd_opt(2022, 3, 15).unwrap())
    );
    assert!(parse_turkish_date("invalid").is_none());
    assert!(parse_turkish_date("").is_none());
}

#[test]
fn year_range_filtering() {
    let d = parse_turkish_date("15.03.2020");
    assert!(decision_matches_year_range(d, None, None));
    assert!(decision_matches_year_range(d, Some(2019), Some(2021)));
    assert!(!decision_matches_year_range(d, Some(2021), Some(2022)));
    assert!(!decision_matches_year_range(d, Some(2018), Some(2019)));
    assert!(!decision_matches_year_range(None, Some(2019), Some(2021)));
}
