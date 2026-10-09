use kukla_lib::core::rag::petition_templates::{all_templates, parse_petition_type};
use kukla_lib::models::PetitionType;

#[test]
fn lists_all_petition_templates() {
    let templates = all_templates();
    assert_eq!(templates.len(), 7);
    assert!(templates.iter().any(|t| t.id == "istinaf"));
}

#[test]
fn parses_petition_type_slugs() {
    assert_eq!(parse_petition_type("genel").unwrap(), PetitionType::Genel);
    assert_eq!(parse_petition_type("cevap").unwrap(), PetitionType::Cevap);
    assert_eq!(parse_petition_type("delil_bildirme").unwrap(), PetitionType::DelilBildirme);
    assert_eq!(parse_petition_type("delil").unwrap(), PetitionType::DelilBildirme);
    assert!(parse_petition_type("bilinmeyen").is_err());
}