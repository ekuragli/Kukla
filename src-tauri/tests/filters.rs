use chrono::NaiveDate;
use kukla_lib::commands::bedesten_search::{decision_matches_year_range, parse_turkish_date};

#[test]
fn parse_turkish_date_handles_dd_mm_yyyy() {
    let date = parse_turkish_date("15.03.2022").unwrap();
    assert_eq!(date, NaiveDate::from_ymd_opt(2022, 3, 15).unwrap());
}

#[test]
fn decision_matches_year_range_filters_correctly() {
    let date = NaiveDate::from_ymd_opt(2021, 6, 1);
    assert!(decision_matches_year_range(date, Some(2020), Some(2022)));
    assert!(!decision_matches_year_range(date, Some(2022), None));
    assert!(decision_matches_year_range(None, None, None));
    assert!(!decision_matches_year_range(None, Some(2020), None));
}
