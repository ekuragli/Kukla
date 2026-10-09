use kukla_lib::utils::path_validation::validate_pdf_path;

#[test]
fn rejects_parent_dir_traversal() {
    let err = validate_pdf_path("..\\secret.pdf").unwrap_err();
    assert!(err.contains("üst dizin"));
}

#[test]
fn rejects_empty_path() {
    let err = validate_pdf_path("").unwrap_err();
    assert!(err.contains("boş"));
}