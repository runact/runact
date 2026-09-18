//! Acceptance tests for Headers type.

use runact_web::headers::Headers;

#[test]
fn test_headers_new_is_empty() {
    let h = Headers::new();
    assert!(h.is_empty());
    assert_eq!(h.len(), 0);
}

#[test]
fn test_headers_insert_and_get() {
    let mut h = Headers::new();
    h.insert("Content-Type", "text/html");
    h.insert("Accept", "application/json");

    assert_eq!(h.get("Content-Type"), Some("text/html"));
    assert_eq!(h.get("Accept"), Some("application/json"));
    assert_eq!(h.len(), 2);
    assert!(!h.is_empty());
}

#[test]
fn test_headers_get_case_insensitive() {
    let mut h = Headers::new();
    h.insert("Content-Type", "text/html");

    assert_eq!(h.get("content-type"), Some("text/html"));
    assert_eq!(h.get("CONTENT-TYPE"), Some("text/html"));
    assert_eq!(h.get("Content-Type"), Some("text/html"));
}

#[test]
fn test_headers_insert_overwrite() {
    let mut h = Headers::new();
    h.insert("Accept", "text/html");
    h.insert("Accept", "application/json");

    assert_eq!(h.get("Accept"), Some("application/json"));
    assert_eq!(h.len(), 1);
}

#[test]
fn test_headers_remove() {
    let mut h = Headers::new();
    h.insert("Accept", "text/html");
    h.insert("Content-Type", "application/json");

    h.remove("Accept");
    assert_eq!(h.get("Accept"), None);
    assert_eq!(h.len(), 1);
}

#[test]
fn test_headers_contains() {
    let mut h = Headers::new();
    h.insert("Host", "example.com");

    assert!(h.contains("Host"));
    assert!(h.contains("host"));
    assert!(!h.contains("Accept"));
}

#[test]
fn test_headers_iter() {
    let mut h = Headers::new();
    h.insert("A", "1");
    h.insert("B", "2");
    h.insert("C", "3");

    let mut pairs: Vec<_> = h.iter().collect();
    pairs.sort();
    assert_eq!(pairs, vec![("a", "1"), ("b", "2"), ("c", "3")]);
}

#[test]
fn test_headers_clear() {
    let mut h = Headers::new();
    h.insert("A", "1");
    h.insert("B", "2");

    h.clear();
    assert!(h.is_empty());
    assert_eq!(h.len(), 0);
}
