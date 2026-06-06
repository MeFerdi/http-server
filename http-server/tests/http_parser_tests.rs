use localhost::http::{parse_request, HttpParseError, Method};

#[test]
fn parses_get_request_line_headers_and_keep_alive() {
    let raw = b"GET /hello HTTP/1.1\r\nHost: example.local\r\nUser-Agent: test\r\n\r\n";
    let req = parse_request(raw, 0).expect("parse GET request");

    assert_eq!(req.method, Method::Get);
    assert_eq!(req.path, "/hello");
    assert_eq!(req.version, "HTTP/1.1");
    assert_eq!(req.header("host"), Some("example.local"));
    assert_eq!(req.body, Vec::<u8>::new());
    assert!(req.keep_alive);
}

#[test]
fn parses_post_with_content_length_body() {
    let raw = b"POST /upload HTTP/1.1\r\nHost: example.local\r\nContent-Length: 11\r\n\r\nhello world";
    let req = parse_request(raw, 0).expect("parse POST request");

    assert_eq!(req.method, Method::Post);
    assert_eq!(req.path, "/upload");
    assert_eq!(req.body, b"hello world".to_vec());
}

#[test]
fn parses_chunked_body_multiple_chunks() {
    let raw = b"POST /chunked HTTP/1.1\r\nHost: example.local\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";
    let req = parse_request(raw, 0).expect("parse chunked request");

    assert_eq!(req.method, Method::Post);
    assert_eq!(req.path, "/chunked");
    assert_eq!(req.body, b"Wikipedia".to_vec());
}

#[test]
fn rejects_malformed_header_missing_colon() {
    let raw = b"GET / HTTP/1.1\r\nHost example.local\r\n\r\n";
    let err = parse_request(raw, 0).expect_err("expected malformed header");

    assert!(matches!(err, HttpParseError::MalformedHeader(_)));
}

#[test]
fn rejects_empty_method() {
    let raw = b" / HTTP/1.1\r\nHost: example.local\r\n\r\n";
    let err = parse_request(raw, 0).expect_err("expected empty method");

    assert!(matches!(err, HttpParseError::EmptyMethod));
}

#[test]
fn rejects_body_exceeding_client_limit() {
    let raw = b"POST /upload HTTP/1.1\r\nHost: example.local\r\nContent-Length: 5\r\n\r\nhello";
    let err = parse_request(raw, 4).expect_err("expected body too large");

    assert!(matches!(
        err,
        HttpParseError::BodyTooLarge {
            limit: 4,
            actual: 5
        }
    ));
}
