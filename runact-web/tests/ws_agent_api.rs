use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

/// Helper: send an HTTP request and read the response.
fn http_request(addr: &str, method: &str, path: &str, body: Option<&str>) -> String {
    let mut client = TcpStream::connect(addr).expect("connect");
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set timeout");

    let request = if let Some(body) = body {
        format!(
            "{method} {path} HTTP/1.1\r\n\
             Host: {addr}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {body}",
            body.len()
        )
    } else {
        format!(
            "{method} {path} HTTP/1.1\r\n\
             Host: {addr}\r\n\
             Connection: close\r\n\
             \r\n"
        )
    };

    client.write_all(request.as_bytes()).expect("write");
    client.flush().expect("flush");

    let mut buf = Vec::new();
    client.read_to_end(&mut buf).expect("read");
    String::from_utf8_lossy(&buf).to_string()
}

/// Integration test for the agent_api REST server.
#[test]
fn test_agent_api_rest() {
    // Spawn agent_api example
    let mut child = std::process::Command::new("cargo")
        .args(["run", "-p", "runact-web", "--example", "agent_api"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("spawn agent_api");

    // Wait for server to start
    thread::sleep(Duration::from_secs(3));

    let addr = "127.0.0.1:8080";

    // GET /api/status
    let resp = http_request(addr, "GET", "/api/status", None);
    assert!(
        resp.contains("200 OK") && resp.contains("agent-api"),
        "status response should contain 200 and service name"
    );

    // POST /api/agents — create session
    let post_body = r#"{"name":"chatbot"}"#;
    let resp = http_request(addr, "POST", "/api/agents", Some(post_body));
    assert!(
        resp.contains("201") && resp.contains("chatbot"),
        "create response should be 201 and contain name"
    );

    // GET /api/agents/1 — get session
    let resp = http_request(addr, "GET", "/api/agents/1", None);
    assert!(
        resp.contains("200") && resp.contains("chatbot"),
        "get session response should contain 200 and name"
    );

    // GET /api/agents/999 — not found
    let resp = http_request(addr, "GET", "/api/agents/999", None);
    assert!(
        resp.contains("404"),
        "non-existent session should return 404"
    );

    // DELETE /api/agents/1
    let resp = http_request(addr, "DELETE", "/api/agents/1", None);
    assert!(
        resp.contains("200") && resp.contains("deleted"),
        "delete response should be 200"
    );

    // DELETE /api/agents/1 again — not found
    let resp = http_request(addr, "DELETE", "/api/agents/1", None);
    assert!(
        resp.contains("404"),
        "deleting non-existent session should return 404"
    );

    // GET /unknown — 404
    let resp = http_request(addr, "GET", "/unknown", None);
    assert!(resp.contains("404"), "unknown path should return 404");

    // Cleanup
    let _ = child.kill();
}
