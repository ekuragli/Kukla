use kukla_lib::commands::open_link::html_escape;

#[test]
fn html_escape_handles_all_dangerous_chars() {
    let input = "<script>'\"&></script>";
    let out = html_escape(input);
    assert!(!out.contains('<'));
    assert!(!out.contains('>'));
    assert!(out.contains("&lt;"));
    assert!(out.contains("&gt;"));
    assert!(out.contains("&quot;"));
    assert!(out.contains("&#39;"));
    assert!(!out.contains('\''));
    assert!(!out.contains('&') || out.contains("&amp;"));
}

#[test]
fn html_escape_preserves_safe_text() {
    let out = html_escape("Mahkeme Kararı 2022/123");
    assert_eq!(out, "Mahkeme Kararı 2022/123");
}
