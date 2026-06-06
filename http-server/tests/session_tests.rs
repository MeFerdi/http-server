use localhost::session::parse_cookie_header;

#[test]
fn parse_cookie_header_extracts_multiple_pairs() {
    let cookies = parse_cookie_header("session_id=abc123; theme=dark; token=xyz");

    assert_eq!(cookies.get("session_id").map(String::as_str), Some("abc123"));
    assert_eq!(cookies.get("theme").map(String::as_str), Some("dark"));
    assert_eq!(cookies.get("token").map(String::as_str), Some("xyz"));
}
