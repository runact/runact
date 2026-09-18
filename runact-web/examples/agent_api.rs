//! REST API server example.
//!
//! A simple REST API server that demonstrates using `runact-web`'s `Router`
//! with `runact`'s TCP listener. Runact's actor runtime can be used to
//! move business logic into supervised actors — see `agent_server.rs`
//! for the full actor integration example.
//!
//! Features:
//! - GET /api/status — health check
//! - GET /api/agents — list registered agents
//! - POST /api/agents — create a new agent session
//! - GET /api/agents/:id — get agent session by ID
//! - DELETE /api/agents/:id — terminate an agent session
//!
//! Run:
//! ```sh
//! cargo run -p runact-web --example agent_api
//! ```
//!
//! Test:
//! ```sh
//! curl http://127.0.0.1:8080/api/status
//! curl -X POST http://127.0.0.1:8080/api/agents -d '{"name":"chatbot"}'
//! curl http://127.0.0.1:8080/api/agents/1
//! curl -X DELETE http://127.0.0.1:8080/api/agents/1
//! ```

use runact_web::headers::Headers;
use runact_web::request::{Method, Request};
use runact_web::response::{Response, StatusCode};
use runact_web::router::Router;

use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// In-memory agent session store (for demonstration).
/// In a real app, this would be an `AgentRegistry` runact Actor.
#[derive(Debug, Clone)]
struct AgentSession {
    id: u64,
    name: String,
}

struct SessionStore {
    sessions: Mutex<Vec<AgentSession>>,
    next_id: AtomicU64,
}

impl SessionStore {
    fn new() -> Self {
        SessionStore {
            sessions: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
        }
    }
}

/// Helper: build a JSON response.
fn json_response(status: StatusCode, body: &str) -> Response {
    let mut headers = Headers::new();
    headers.insert("Content-Type", "application/json");
    headers.insert("Content-Length", &body.len().to_string());
    Response {
        status,
        reason: status.reason().to_string(),
        version: "HTTP/1.1".to_string(),
        headers,
        body: body.as_bytes().to_vec(),
    }
}

/// Extract a string value for a JSON key from a body like `{"key":"value"}`.
fn extract_json_string(body: &str, key: &str) -> Option<String> {
    let search = format!("\"{key}\"");
    let after = body.split(&search).nth(1)?;
    let after_colon = after.split(':').nth(1)?;
    let rest = after_colon.trim_start();
    rest.strip_prefix('"')?
        .split('"')
        .next()
        .map(|s| s.to_string())
}

fn main() {
    let listener = runact::net::tcp_api::TcpListener::bind("127.0.0.1:8080").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    eprintln!("agent_api: listening on {addr}");
    eprintln!("agent_api: try: curl http://{addr}/api/status");

    // Shared session store (would be an Actor in a real app)
    let store = Arc::new(SessionStore::new());

    // Build the router with all handlers
    let mut router = Router::new();

    // GET /api/status
    router.route(Method::Get, "/api/status", |_req| {
        json_response(StatusCode::OK, r#"{"status":"ok","service":"agent-api"}"#)
    });

    // GET /api/agents — list sessions
    let store_clone = Arc::clone(&store);
    router.route(Method::Get, "/api/agents", move |_req| {
        let json: Vec<String> = store_clone
            .sessions
            .lock()
            .unwrap()
            .iter()
            .map(|s| format!(r#"{{"id":{},"name":"{}"}}"#, s.id, s.name))
            .collect();
        let body = format!(
            r#"{{"count":{count},"sessions":[{}]}}"#,
            json.join(","),
            count = store_clone.sessions.lock().unwrap().len()
        );
        json_response(StatusCode::OK, &body)
    });

    // POST /api/agents — create session
    let store_clone = Arc::clone(&store);
    router.route(Method::Post, "/api/agents", move |req: Request| {
        let body = String::from_utf8_lossy(&req.body);
        let name = extract_json_string(&body, "name").unwrap_or_else(|| "unnamed".to_string());
        let id = store_clone.next_id.fetch_add(1, Ordering::SeqCst);
        store_clone.sessions.lock().unwrap().push(AgentSession {
            id,
            name: name.clone(),
        });
        let resp_body = format!(r#"{{"id":{id},"name":"{name}","created":true}}"#);
        json_response(StatusCode::Created, &resp_body)
    });

    // GET /api/agents/:id
    let store_clone = Arc::clone(&store);
    router.route(Method::Get, "/api/agents/:id", move |req: Request| {
        let id: u64 = req.param("id").unwrap_or("0").parse().unwrap_or(0);
        let sessions = store_clone.sessions.lock().unwrap();
        match sessions.iter().find(|s| s.id == id) {
            Some(s) => {
                let body = format!(r#"{{"id":{id},"name":"{}"}}"#, s.name);
                json_response(StatusCode::OK, &body)
            }
            None => json_response(StatusCode::NotFound, r#"{"error":"not found"}"#),
        }
    });

    // DELETE /api/agents/:id
    let store_clone = Arc::clone(&store);
    router.route(Method::Delete, "/api/agents/:id", move |req: Request| {
        let id: u64 = req.param("id").unwrap_or("0").parse().unwrap_or(0);
        let mut sessions = store_clone.sessions.lock().unwrap();
        let len_before = sessions.len();
        sessions.retain(|s| s.id != id);
        let deleted = sessions.len() < len_before;
        if deleted {
            json_response(StatusCode::OK, r#"{"deleted":true}"#)
        } else {
            json_response(StatusCode::NotFound, r#"{"error":"not found"}"#)
        }
    });

    // 404 handler
    router.not_found(|_req| json_response(StatusCode::NotFound, r#"{"error":"not found"}"#));

    let router = Arc::new(router);

    // Accept connections and handle as HTTP
    loop {
        let stream = match listener.accept() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("agent_api: accept error: {e}");
                continue;
            }
        };

        let adapter = runact_web::websocket::runact_tcp::RunactTcpStream(stream);
        let router = Arc::clone(&router);

        let _ = std::thread::spawn(move || {
            let mut stream = adapter;

            // Read the HTTP request
            let mut buf = [0u8; 8192];
            let n = match read_http_request(&mut stream, &mut buf) {
                Ok(n) => n,
                Err(_) => return,
            };

            // Parse and route
            let req = match Request::parse(&buf[..n]) {
                Ok(r) => r,
                Err(_) => {
                    let resp = json_response(StatusCode::BadRequest, r#"{"error":"bad request"}"#);
                    let _ = stream.write_all(&resp.encode());
                    return;
                }
            };

            let resp = router.handle(req);
            let _ = stream.write_all(&resp.encode());
        });
    }
}

/// Read a complete HTTP request from the stream.
fn read_http_request<S: Read>(stream: &mut S, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    let mut headers_complete = false;
    let mut content_length: usize = 0;
    let mut content_length_set = false;

    loop {
        let n = stream.read(&mut buf[total..])?;
        if n == 0 {
            break;
        }
        total += n;

        if !headers_complete {
            if let Some(pos) = find_header_end(&buf[..total]) {
                headers_complete = true;
                let header_str = std::str::from_utf8(&buf[..pos]).unwrap_or("");
                for line in header_str.lines() {
                    if let Some(cl_str) = line.strip_prefix("Content-Length:") {
                        if let Ok(cl) = cl_str.trim().parse::<usize>() {
                            content_length = cl;
                            content_length_set = true;
                        }
                    }
                }
            }
        }

        if headers_complete && content_length_set {
            if total >= content_length {
                break;
            }
        } else if headers_complete && !content_length_set {
            break;
        }
    }

    Ok(total)
}

/// Find the end of HTTP headers (double CRLF).
fn find_header_end(data: &[u8]) -> Option<usize> {
    if data.len() < 4 {
        return None;
    }
    for i in 0..=(data.len() - 4) {
        if &data[i..i + 4] == b"\r\n\r\n" {
            return Some(i);
        }
    }
    None
}
