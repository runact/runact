//! Acceptance tests for HTTP Response parsing and construction.

use runact_web::headers::Headers;
use runact_web::response::{Response, StatusCode};

#[test]
fn test_parse_simple_response() {
    let raw =
        b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\nContent-Type: text/plain\r\n\r\nHello, World!";
    let resp = Response::parse(raw).unwrap();

    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.reason, "OK");
    assert_eq!(resp.version, "HTTP/1.1");
    assert_eq!(resp.headers.get("Content-Length"), Some("13"));
    assert_eq!(resp.headers.get("Content-Type"), Some("text/plain"));
    assert_eq!(resp.body, b"Hello, World!");
}

#[test]
fn test_parse_response_404() {
    let raw = b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found";
    let resp = Response::parse(raw).unwrap();

    assert_eq!(resp.status, StatusCode::NotFound);
    assert_eq!(resp.reason, "Not Found");
    assert_eq!(resp.body, b"Not Found");
}

#[test]
fn test_parse_response_500() {
    let raw =
        b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 21\r\n\r\nInternal Server Error";
    let resp = Response::parse(raw).unwrap();

    assert_eq!(resp.status, StatusCode::InternalServerError);
}

#[test]
fn test_parse_response_no_body() {
    let raw = b"HTTP/1.1 204 No Content\r\n\r\n";
    let resp = Response::parse(raw).unwrap();

    assert_eq!(resp.status, StatusCode::NoContent);
    assert!(resp.body.is_empty());
}

#[test]
fn test_parse_response_headers_case_insensitive() {
    let raw = b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n";
    let resp = Response::parse(raw).unwrap();

    assert_eq!(resp.headers.get("Content-Length"), Some("0"));
    assert_eq!(resp.headers.get("content-length"), Some("0"));
}

#[test]
fn test_parse_response_fails_on_bad_status_line() {
    let raw = b"HTTP/1.1 OK\r\n\r\n";
    assert!(Response::parse(raw).is_err());
}

#[test]
fn test_response_construction() {
    let mut headers = Headers::new();
    headers.insert("Content-Type", "application/json");

    let resp = Response {
        status: StatusCode::OK,
        reason: "OK".to_string(),
        version: "HTTP/1.1".to_string(),
        headers,
        body: b"{\"ok\":true}".to_vec(),
    };

    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.body, b"{\"ok\":true}");
}

#[test]
fn test_status_code_display() {
    assert_eq!(StatusCode::OK.to_string(), "200");
    assert_eq!(StatusCode::NotFound.to_string(), "404");
    assert_eq!(StatusCode::InternalServerError.to_string(), "500");
    assert_eq!(StatusCode::BadRequest.to_string(), "400");
    assert_eq!(StatusCode::Unauthorized.to_string(), "401");
}

#[test]
fn test_status_code_reason() {
    assert_eq!(StatusCode::OK.reason(), "OK");
    assert_eq!(StatusCode::NotFound.reason(), "Not Found");
    assert_eq!(
        StatusCode::InternalServerError.reason(),
        "Internal Server Error"
    );
    assert_eq!(StatusCode::BadRequest.reason(), "Bad Request");
}

#[test]
fn test_status_code_from_u16() {
    assert_eq!(StatusCode::from_u16(200).unwrap(), StatusCode::OK);
    assert_eq!(StatusCode::from_u16(404).unwrap(), StatusCode::NotFound);
    assert_eq!(
        StatusCode::from_u16(500).unwrap(),
        StatusCode::InternalServerError
    );
    assert!(StatusCode::from_u16(999).is_err());
}

#[test]
fn test_response_encode() {
    let mut headers = Headers::new();
    headers.insert("Content-Length", "5");

    let resp = Response {
        status: StatusCode::OK,
        reason: "OK".to_string(),
        version: "HTTP/1.1".to_string(),
        headers,
        body: b"hello".to_vec(),
    };

    let encoded = resp.encode();
    let expected = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
    assert_eq!(encoded, expected);
}

#[test]
fn test_response_encode_no_body() {
    let resp = Response {
        status: StatusCode::NoContent,
        reason: "No Content".to_string(),
        version: "HTTP/1.1".to_string(),
        headers: Headers::new(),
        body: vec![],
    };

    let encoded = resp.encode();
    let expected = b"HTTP/1.1 204 No Content\r\n\r\n";
    assert_eq!(encoded, expected);
}

#[test]
fn test_response_encode_with_multiple_headers() {
    let mut headers = Headers::new();
    headers.insert("Content-Type", "text/html");
    headers.insert("Content-Length", "15");
    headers.insert("X-Request-Id", "abc-123");

    let resp = Response {
        status: StatusCode::OK,
        reason: "OK".to_string(),
        version: "HTTP/1.1".to_string(),
        headers,
        body: b"<h1>Hello</h1>".to_vec(),
    };

    let encoded = resp.encode();
    assert!(encoded.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert!(encoded.ends_with(b"\r\n\r\n<h1>Hello</h1>"));
    // Headers must be present (order may vary due to internal storage)
    let s = String::from_utf8_lossy(&encoded);
    assert!(s.contains("Content-Type: text/html\r\n"));
    assert!(s.contains("Content-Length: 15\r\n"));
    assert!(s.contains("X-Request-Id: abc-123\r\n"));
}

#[test]
fn test_custom_status_code() {
    let raw = b"HTTP/1.1 418 I'm a Teapot\r\nContent-Length: 0\r\n\r\n";
    let resp = Response::parse(raw).unwrap();

    assert_eq!(resp.status, StatusCode::Custom(418));
    assert_eq!(resp.reason, "I'm a Teapot");
}
