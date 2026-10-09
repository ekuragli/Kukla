use kukla_lib::commands::export::{char_count, wrap_text_line};

#[test]
fn wraps_long_lines_for_pdf() {
    let long = "Bu metin PDF sayfasına sığmayacak kadar uzun bir cümledir ve satır sonlarında düzgün şekilde bölünmesi gerekir.";
    let wrapped = wrap_text_line(long, 40);
    assert!(wrapped.len() > 1);
    for line in &wrapped {
        assert!(char_count(line) <= 40);
    }
}