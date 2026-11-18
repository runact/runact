use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Pick a free ephemeral address for the example server.
fn free_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    listener.local_addr().expect("local addr").to_string()
}

/// Path to a prebuilt example binary in the workspace target directory.
fn example_binary(name: &str) -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("workspace root")
                .join("target")
        });
    target.join("debug").join("examples").join(name)
}

/// Kills the example server when the test ends, including on panic.
struct ServerGuard(Child);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Wait until the server accepts connections (max ~5s).
fn wait_until_ready(addr: &str) {
    for _ in 0..50 {
        if TcpStream::connect(addr).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("server at {addr} did not become ready");
}

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
    // Run the prebuilt example directly. Invoking `cargo run` from inside
    // `cargo test` would block on the build lock held by the test harness.
    let addr = free_addr();
    let binary = example_binary("agent_api");
    assert!(
        binary.exists(),
        "missing {} — build examples first (`cargo build -p runact-web --examples`)",
        binary.display()
    );

    let child = Command::new(&binary)
        .env("AGENT_API_ADDR", &addr)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn agent_api");
    let _guard = ServerGuard(child);

    wait_until_ready(&addr);

    // GET /api/status
    let resp = http_request(&addr, "GET", "/api/status", None);
    assert!(
        resp.contains("200 OK") && resp.contains("agent-api"),
        "status response should contain 200 and service name"
    );

    // POST /api/agents — create session
    let post_body = r#"{"name":"chatbot"}"#;
    let resp = http_request(&addr, "POST", "/api/agents", Some(post_body));
    assert!(
        resp.contains("201") && resp.contains("chatbot"),
        "create response should be 201 and contain name"
    );

    // GET /api/agents/1 — get session
    let resp = http_request(&addr, "GET", "/api/agents/1", None);
    assert!(
        resp.contains("200") && resp.contains("chatbot"),
        "get session response should contain 200 and name"
    );

    // GET /api/agents/999 — not found
    let resp = http_request(&addr, "GET", "/api/agents/999", None);
    assert!(
        resp.contains("404"),
        "non-existent session should return 404"
    );

    // DELETE /api/agents/1
    let resp = http_request(&addr, "DELETE", "/api/agents/1", None);
    assert!(
        resp.contains("200") && resp.contains("deleted"),
        "delete response should be 200"
    );

    // DELETE /api/agents/1 again — not found
    let resp = http_request(&addr, "DELETE", "/api/agents/1", None);
    assert!(
        resp.contains("404"),
        "deleting non-existent session should return 404"
    );

    // GET /unknown — 404
    let resp = http_request(&addr, "GET", "/unknown", None);
    assert!(resp.contains("404"), "unknown path should return 404");
}
