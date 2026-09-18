//! Acceptance tests for HTTP Request parsing and construction.

use runact_web::headers::Headers;
use runact_web::request::{Method, Request};

#[test]
fn test_parse_simple_get_request() {
    let raw = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\nAccept: text/html\r\n\r\n";
    let req = Request::parse(raw).unwrap();

    assert_eq!(req.method, Method::Get);
    assert_eq!(req.path, "/index.html");
    assert_eq!(req.version, "HTTP/1.1");
    assert_eq!(req.headers.get("Host"), Some("example.com"));
    assert_eq!(req.headers.get("Accept"), Some("text/html"));
    assert!(req.body.is_empty());
}

#[test]
fn test_parse_post_request_with_body() {
    let raw = b"POST /api/users HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/json\r\nContent-Length: 38\r\n\r\n{\"name\":\"Alice\",\"email\":\"a@b.com\"}";
    let req = Request::parse(raw).unwrap();

    assert_eq!(req.method, Method::Post);
    assert_eq!(req.path, "/api/users");
    assert_eq!(req.headers.get("Content-Type"), Some("application/json"));
    assert_eq!(req.body, b"{\"name\":\"Alice\",\"email\":\"a@b.com\"}");
}

#[test]
fn test_parse_request_with_multiple_headers() {
    let raw = b"GET /api/data HTTP/1.1\r\nHost: example.com\r\nAccept: application/json\r\nAuthorization: Bearer token123\r\nUser-Agent: runact-web/0.1\r\n\r\n";
    let req = Request::parse(raw).unwrap();

    assert_eq!(req.headers.get("Authorization"), Some("Bearer token123"));
    assert_eq!(req.headers.get("User-Agent"), Some("runact-web/0.1"));
}

#[test]
fn test_parse_request_headers_case_insensitive() {
    let raw = b"GET / HTTP/1.1\r\nHost: example.com\r\ncontent-type: text/plain\r\n\r\n";
    let req = Request::parse(raw).unwrap();

    assert_eq!(req.headers.get("Content-Type"), Some("text/plain"));
    assert_eq!(req.headers.get("content-type"), Some("text/plain"));
    assert_eq!(req.headers.get("CONTENT-TYPE"), Some("text/plain"));
}

#[test]
fn test_parse_request_fails_on_invalid_method() {
    let raw = b"INVALID / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    assert!(Request::parse(raw).is_err());
}

#[test]
fn test_parse_request_fails_on_missing_headers() {
    let raw = b"GET / HTTP/1.1\r\n\r\n";
    // Missing Host header should fail
    assert!(Request::parse(raw).is_err());
}

#[test]
fn test_parse_request_fails_on_malformed_start_line() {
    let raw = b"GET / HTTP/1.1 extra\r\nHost: example.com\r\n\r\n";
    assert!(Request::parse(raw).is_err());
}

#[test]
fn test_request_construction() {
    let mut headers = Headers::new();
    headers.insert("Host", "example.com");
    headers.insert("Accept", "application/json");

    let req = Request {
        method: Method::Get,
        path: "/api/status".to_string(),
        version: "HTTP/1.1".to_string(),
        headers,
        body: vec![],
    };

    assert_eq!(req.method, Method::Get);
    assert_eq!(req.path, "/api/status");
}

#[test]
fn test_method_display() {
    assert_eq!(Method::Get.to_string(), "GET");
    assert_eq!(Method::Post.to_string(), "POST");
    assert_eq!(Method::Put.to_string(), "PUT");
    assert_eq!(Method::Delete.to_string(), "DELETE");
    assert_eq!(Method::Patch.to_string(), "PATCH");
    assert_eq!(Method::Head.to_string(), "HEAD");
    assert_eq!(Method::Options.to_string(), "OPTIONS");
}

#[test]
fn test_method_from_str() {
    assert_eq!("GET".parse::<Method>().unwrap(), Method::Get);
    assert_eq!("POST".parse::<Method>().unwrap(), Method::Post);
    assert_eq!("PUT".parse::<Method>().unwrap(), Method::Put);
    assert_eq!("DELETE".parse::<Method>().unwrap(), Method::Delete);
    assert!("INVALID".parse::<Method>().is_err());
}

#[test]
fn test_request_encode() {
    let mut headers = Headers::new();
    headers.insert("Host", "example.com");

    let req = Request {
        method: Method::Get,
        path: "/".to_string(),
        version: "HTTP/1.1".to_string(),
        headers,
        body: vec![],
    };

    let encoded = req.encode();
    let expected = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    assert_eq!(encoded, expected);
}

#[test]
fn test_request_encode_with_body() {
    let mut headers = Headers::new();
    headers.insert("Content-Type", "text/plain");
    headers.insert("Content-Length", "5");

    let req = Request {
        method: Method::Post,
        path: "/submit".to_string(),
        version: "HTTP/1.1".to_string(),
        headers,
        body: b"hello".to_vec(),
    };

    let encoded = req.encode();
    assert!(encoded.starts_with(b"POST /submit HTTP/1.1\r\n"));
    assert!(encoded.ends_with(b"\r\n\r\nhello"));
}
