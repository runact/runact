# Chapter 17: REST APIs

This chapter walks through the `agent_api.rs` example: a REST API server
using `runact-web`'s `Router` with runact's TCP listener.

---

## 17.1 Architecture

```
HTTP Client
    │ (HTTP/1.1 over TCP)
    ▼
TcpListener (runact::net)
    │ (accept each connection)
    ▼
Thread per connection
    │ (read HTTP request)
    ▼
Router::handle(Request) → Response
    │
    ▼
TcpStream (write HTTP response)
```

## 17.2 The Router

The `Router` matches HTTP requests to handlers based on method and path
patterns:

```rust
use runact_web::router::Router;
use runact_web::request::{Method, Request};
use runact_web::response::{Response, StatusCode};

let mut router = Router::new();

router.route(Method::Get, "/api/agents", |req| {
    Response {
        status: StatusCode::OK,
        body: b"[]".to_vec(),
        ..Default::default()
    }
});

router.route(Method::Post, "/api/agents", |req| {
    // Create an agent
    Response {
        status: StatusCode::Created,
        body: req.body.clone(),
        ..Default::default()
    }
});

router.route(Method::Get, "/api/agents/:id", |req| {
    let id = req.param("id").unwrap_or("0");
    Response {
        status: StatusCode::OK,
        body: format!(r#"{{"id":"{id}"}}""#).into_bytes(),
        ..Default::default()
    }
});
```

### Path Parameters

Routes can capture path segments with `:name`:

```rust
router.route(Method::Get, "/api/agents/:id/messages/:msg_id", |req| {
    let agent_id = req.param("id").unwrap();
    let msg_id = req.param("msg_id").unwrap();
    // ...
});
```

## 17.3 Shared State

For a REST API server, you need shared state (e.g., a session store).
Since each HTTP connection runs on its own thread, use `Arc<Mutex<…>>`:

```rust
use std::sync::{Arc, Mutex};

struct SessionStore {
    sessions: Mutex<Vec<Session>>,
}

struct Session {
    id: u64,
    name: String,
}

let store = Arc::new(SessionStore {
    sessions: Mutex::new(Vec::new()),
});

// Pass `Arc::clone(&store)` to each connection thread
let store_clone = Arc::clone(&store);
let response = router.handle(request);
```

> **Note:** In a production system, this shared state would be an actor
> managed by the runact runtime (see Chapter 16). The `Arc<Mutex>` pattern
> here is for the minimal example. Using an actor gives you message-based
> concurrency, supervision, and mailbox backpressure.

## 17.4 Reading HTTP Requests

HTTP requests are parsed from raw TCP bytes:

```rust
use runact_web::request::Request;
use std::io::Read;

let mut buf = [0u8; 4096];
let n = client.read(&mut buf)?;
let request = Request::parse(&buf[..n])?;
```

`Request::parse` handles:
- Start line (method, path, version)
- Headers (including `Content-Length`)
- Body extraction

## 17.5 The Full Server Loop

```rust
use runact::net::tcp_api::TcpListener;
use runact_web::router::Router;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let store = Arc::new(SessionStore::new());
    let router = build_router(Arc::clone(&store));

    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    loop {
        let mut client = listener.accept().unwrap();
        let router = Arc::new(router);

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let n = client.read(&mut buf).unwrap();
            let request = Request::parse(&buf[..n]).unwrap();
            let response = router.handle(request);
            let bytes = response.encode();
            client.write_all(&bytes).unwrap();
            client.flush().unwrap();
        });
    }
}
```

### Why Thread-Per-Connection?

This is the simplest possible HTTP server. For production use, you would
instead:

1. Use the **actor model** — spawn a per-connection actor for each TCP
   connection.
2. Use **async tasks** — spawn an async task per connection that uses
   `runact::Runtime::sleep` and async I/O.
3. Use an **actor-per-connection** pattern with reader/writer threads,
   just like the WebSocket server (Chapter 14).

## 17.6 JSON Helpers

Since runact-web has no `serde` dependency, JSON is constructed manually:

```rust
fn json_response(status: StatusCode, body: &str) -> Response {
    Response {
        status,
        reason: status.reason().to_string(),
        version: "HTTP/1.1".to_string(),
        headers: Headers::new(),
        body: body.as_bytes().to_vec(),
    }
}
```

For more complex JSON, consider adding `serde_json` as a dependency in
your application (runact-web core stays dependency-free).

## 17.7 Testing

The `ws_agent_api.rs` integration test verifies the full REST lifecycle:

```rust
// GET /api/health → 200 "ok"
// POST /api/agents → 201 (creates session)
// GET /api/agents → 200 (lists sessions)
// GET /api/agents/:id → 200 (get single session)
// DELETE /api/agents/:id → 200 (delete session)
// GET /api/agents/:id (deleted) → 404
```

See `tests/ws_agent_api.rs` for the test implementation.
