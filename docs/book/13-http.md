# Chapter 13: HTTP with runact-web

`runact-web` provides minimal HTTP/1.1 support: `Request` and `Response`
types, `Headers`, and a `Router` with path-parameter matching. This chapter
shows how to build a simple HTTP server on top of runact's TCP layer.

---

## 13.1 The Request Type

```rust
pub struct Request {
    pub method: Method,
    pub path: String,
    pub version: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}
```

### Parsing from Bytes

```rust
use runact_web::request::Request;
use runact::net::tcp_api::TcpListener;
use std::io::Read;
use std::thread;

let listener = TcpListener::bind("127.0.0.1:8080")?;

loop {
    let mut client = listener.accept()?;
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let n = client.read(&mut buf).unwrap();
        // `read` maps WouldBlock to Ok(0); retry if no bytes are available yet.
        if n == 0 {
            return;
        }
        let request = Request::parse(&buf[..n]).unwrap();
        println!("{} {}", request.method, request.path);
    });
}
```

### Path Parameters

The router sets path parameters on the `Request` before handing it to the
handler. You can read them with `request.param("name")`:

```rust
let id = request.param("id").unwrap_or("unknown");
```

## 13.2 The Response Type

```rust
pub struct Response {
    pub status: StatusCode,
    pub reason: String,
    pub version: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}
```

### Encoding

```rust
let bytes = response.encode();
client.write_all(&bytes)?;
client.flush()?;
```

## 13.3 The Router

```rust
use runact_web::headers::Headers;
use runact_web::request::Method;
use runact_web::response::{Response, StatusCode};
use runact_web::router::Router;

let mut router = Router::new();

// Static path
router.route(Method::Get, "/", |_req| {
    Response {
        status: StatusCode::OK,
        reason: "OK".to_string(),
        version: "HTTP/1.1".to_string(),
        headers: Headers::new(),
        body: b"Welcome!".to_vec(),
    }
});

// Path with parameter
router.route(Method::Get, "/users/:id", |req| {
    let id = req.param("id").unwrap_or("0");
    let body = format!(r#"{{"id": "{id}"}}"#).into_bytes();
    Response {
        status: StatusCode::OK,
        reason: "OK".to_string(),
        version: "HTTP/1.1".to_string(),
        headers: Headers::new(),
        body,
    }
});

// POST with body
router.route(Method::Post, "/users", |req| {
    Response {
        status: StatusCode::Created,
        reason: "Created".to_string(),
        version: "HTTP/1.1".to_string(),
        headers: Headers::new(),
        body: req.body,
    }
});

// 404 handler
router.not_found(|_req| {
    Response {
        status: StatusCode::NotFound,
        reason: "Not Found".to_string(),
        version: "HTTP/1.1".to_string(),
        headers: Headers::new(),
        body: b"Not Found".to_vec(),
    }
});
```

> **Note:** `Response` has no `Default` impl. Construct all five fields, or
> return a `&str`, `StatusCode`, or `(StatusCode, Vec<u8>)` from a handler
> via the `IntoResponse` trait (the router's `Handler` bound still expects a
> `Response`, so call `.into_response()` explicitly).

### Router::handle

```rust
let response = router.handle(request);
```

Routes are matched in registration order (first match wins). Path
patterns use `:name` for parameters: `/users/:id`, `/users/:user_id/posts/:post_id`.

## 13.4 Handler Trait

Any `Fn(Request) -> Response + Send + Sync + 'static` automatically
implements the `Handler` trait. This means closures work directly:

```rust
router.route(Method::Get, "/health", |_| health_response());
```

## 13.5 Building a Complete HTTP Server

```rust
use runact::net::tcp_api::TcpListener;
use runact_web::headers::Headers;
use runact_web::request::{Method, Request};
use runact_web::response::{Response, StatusCode};
use runact_web::router::Router;
use std::io::{Read, Write};
use std::thread;

fn build_router() -> Router {
    let mut router = Router::new();
    router.route(Method::Get, "/health", |_| {
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: b"healthy".to_vec(),
        }
    });
    router
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    let router = build_router();

    loop {
        let mut client = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = client.read(&mut buf).unwrap();
        let req = Request::parse(&buf[..n]).unwrap();
        let resp = router.handle(req);
        let bytes = resp.encode();
        client.write_all(&bytes).unwrap();
        client.flush().unwrap();
    }
}
```

## 13.6 IntoResponse

The `IntoResponse` trait converts values into `Response`:

```rust
impl IntoResponse for Response { ... }
impl IntoResponse for &str { ... }
impl IntoResponse for StatusCode { ... }
impl IntoResponse for (StatusCode, Vec<u8>) { ... }
```

This lets handlers return idiomatic types:

```rust
router.route(Method::Get, "/users/:id", |req| {
    let id = req.param("id").unwrap_or("0");
    (StatusCode::OK, format!(r#"{{"id":"{id}"}}""#).into_bytes())
});
```
