# Chapter 14: WebSocket Server

`runact-web::websocket` provides a `WebSocketServer` that wraps any stream
implementing `Read + Write + SetReadTimeout`, plus an `AsyncWebSocket`
that handles the RFC 6455 protocol internals. This chapter covers the
server API and how to build WebSocket applications.

---

## 14.1 WebSocketServer

```rust
use runact_web::websocket::server::WebSocketServer;
use runact_web::websocket::server::ServerEvent;

let server = WebSocketServer::bind("127.0.0.1:8080/ws")?;

server.accept_with_callback_and_config(
    |writer, event| {
        match event {
            ServerEvent::Connected => {
                println!("Client connected");
            }
            ServerEvent::Frame(frame) => {
                writer.send_text(&frame.payload_as_text()).unwrap();
            }
            ServerEvent::Closed => {
                println!("Client disconnected");
            }
            ServerEvent::Error(e) => {
                eprintln!("Error: {}", e);
            }
        }
    },
    WebSocketConfig { ping_interval: Some(Duration::from_secs(30)) },
)?;
```

### ServerEvent

The callback receives `ServerEvent` variants:

| Variant | When | Description |
|---------|------|-------------|
| `Connected` | Before reader loop | Client completed the WebSocket handshake |
| `Frame(Frame)` | Each frame | A parsed WebSocket frame |
| `Closed` | Connection closed | The peer closed the connection |
| `Error(HandshakeError)` | Handshake failure | Invalid or malformed upgrade request |

### WebSocketConfig

```rust
pub struct WebSocketConfig {
    pub ping_interval: Option<Duration>,
}
```

When `ping_interval` is set, a background thread sends Ping frames at the
configured interval. The peer must respond with Pong frames (auto-handled
by the `AsyncWebSocket` reader).

## 14.2 Stream Abstraction

`WebSocketServer` is generic over `S: Read + Write + SetReadTimeout + Send + 'static`.
This allows it to work with both standard TCP streams and runact's native
TCP streams:

### Standard TCP

```rust
let server = WebSocketServer::<std::net::TcpStream>::bind("127.0.0.1:8080/ws")?;
```

### Runact TCP Bridge

```rust
use runact_web::websocket::runact_tcp::RunactTcpStream;

let server = WebSocketServer::<RunactTcpStream>::bind("127.0.0.1:8080/ws")?;
```

The `RunactTcpStream` adapter implements `Read`, `Write`, and
`SetReadTimeout` (no-op, since runact's TCP is non-blocking with internal
retry loops).

## 14.3 Frame API

The `Frame` struct represents a single WebSocket frame:

```rust
pub struct Frame {
    pub fin: bool,           // Is this the final frame in a message?
    pub opcode: OpCode,      // Text, Binary, Ping, Pong, Close, Continuation
    pub masked: bool,        // Was the frame masked?
    pub mask_key: [u8; 4],   // Masking key (if masked)
    pub payload: Vec<u8>,    // Frame payload
}
```

### OpCode

```rust
pub enum OpCode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}
```

## 14.4 ConnectionWriter

The `ConnectionWriter` passed to the callback is how you send data back
to the client:

```rust
impl ConnectionWriter {
    pub fn send_text(&self, text: &str) -> Result<(), SendError>;
    pub fn send_binary(&self, data: &[u8]) -> Result<(), SendError>;
    pub fn send_ping(&self, data: &[u8]) -> Result<(), SendError>;
    pub fn send_pong(&self, data: &[u8]) -> Result<(), SendError>;
    pub fn send_close(&self, code: u16) -> Result<(), SendError>;
}
```

### SendError

```rust
pub enum SendError {
    Full,    // Channel to writer thread is full
    Closed,  // Connection has been closed
}
```

## 14.5 Echo Server Example

```rust
use runact_web::websocket::server::{ServerEvent, WebSocketServer};
use runact_web::websocket::WebSocketConfig;
use std::time::Duration;

let server = WebSocketServer::bind("127.0.0.1:8080/ws")?;

server.accept_with_callback_and_config(
    |writer, event| {
        match event {
            ServerEvent::Connected => println!("Client connected"),
            ServerEvent::Frame(frame) => {
                // Echo back any text or binary frame
                if !frame.fin || matches!(frame.opcode, OpCode::Text | OpCode::Binary) {
                    let _ = writer.send_binary(&frame.payload);
                }
            }
            ServerEvent::Closed => println!("Client disconnected"),
            ServerEvent::Error(e) => eprintln!("Error: {}", e),
        }
    },
    WebSocketConfig { ping_interval: Some(Duration::from_secs(30)) },
)?;
```

## 14.6 Fragmentation Handling

The reader loop automatically reassembles fragmented messages per
RFC 6455 §5.4:

- **First frame**: `FIN=false`, opcode = `Text` or `Binary`
- **Continuation frames**: `FIN=false`, opcode = `Continuation`
- **Final frame**: `FIN=true`, opcode = `Continuation`

Control frames (`Ping`, `Pong`, `Close`) can interleave with fragmented
data — they are delivered immediately and never disrupt reassembly.

The reassembled frame is delivered with `FIN=true` and the original
data opcode. See Chapter 15 for details.

## 14.7 Handshake

The WebSocket upgrade handshake is automatic:

1. Client sends HTTP request with `Upgrade: websocket` header and
   `Sec-WebSocket-Key` header.
2. Server validates the request and responds with
   `HTTP/1.1 101 Switching Protocols` and a computed
   `Sec-WebSocket-Accept` header.
3. The connection switches to the WebSocket framing protocol.

The `ServerEvent::Connected` variant fires **before** the reader loop
starts, ensuring the application knows the connection is established.
