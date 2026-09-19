# runact-web

HTTP/WebSocket layer for the [Runact](https://crates.io/crates/runact) actor runtime.

## Features

- **HTTP/1.1** request parsing, response construction, headers
- **Router** with path parameters (`:id`) and middleware chains
- **WebSocket** (RFC 6455): frame parsing/encoding, handshake validation, ping/pong heartbeats, message fragmentation/reassembly, TCP bridge adapter, echo/chat/agent server examples

## Quick Start

### Dependencies

```toml
[dependencies]
runact = "1.2"
runact-web = "0.2"
sha1 = "0.10"
base64 = "0.22"
```

### WebSocket Echo Server

```rust
use runact::net::tcp_api::TcpListener;
use runact_web::websocket::server::{WebSocketServer, ServerEvent};
use runact_web::websocket::WebSocketConfig;
use std::time::Duration;

let server = WebSocketServer::bind("127.0.0.1:8080/ws")?;
server.accept_with_callback_and_config(
    |writer, event| {
        match event {
            ServerEvent::Connected => println!("Client connected"),
            ServerEvent::Frame(frame) => {
                let _ = writer.send_binary(&frame.payload);
            }
            ServerEvent::Closed => println!("Client disconnected"),
            ServerEvent::Error(e) => eprintln!("Error: {}", e),
        }
    },
    WebSocketConfig { ping_interval: Some(Duration::from_secs(30)) },
)?;
```

### REST API

```rust
use runact::net::tcp_api::TcpListener;
use runact_web::router::Router;
use runact_web::request::{Method, Request};
use runact_web::response::{Response, StatusCode};

let mut router = Router::new();
router.route(Method::Get, "/api/health", |_| {
    Response {
        status: StatusCode::OK,
        body: b"ok".to_vec(),
        ..Default::default()
    }
});
```

## Examples

```bash
# WebSocket echo server (127.0.0.1:8080/ws)
cargo run --example websocket_echo

# WebSocket broadcast chat (127.0.0.1:8080/chat)
cargo run --example websocket_chat

# AI agent server (127.0.0.1:8080/ws)
cargo run --example agent_server

# REST API server (127.0.0.1:8080)
cargo run --example agent_api
```

## Testing

```bash
cargo test -p runact-web --all-targets
```

## License

MIT OR Apache-2.0
