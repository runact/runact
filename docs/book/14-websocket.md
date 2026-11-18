# Chapter 14: WebSocket Server

`runact-web::websocket` provides a `WebSocketServer` that wraps any stream
implementing `Read + Write + SetReadTimeout`, plus an `AsyncWebSocket`
that handles the RFC 6455 protocol internals. This chapter covers the
server API and how to build WebSocket applications.

---

## 14.1 WebSocketServer

`WebSocketServer::bind` takes an **accepted stream** plus the path to match;
it does not bind a listening socket. Accept connections yourself, then hand
each one to the server:

```rust
use runact_web::websocket::async_ws::WebSocketConfig;
use runact_web::websocket::server::{ServerEvent, WebSocketServer};
use std::time::Duration;

let listener = std::net::TcpListener::bind("127.0.0.1:8080")?;

for stream in listener.incoming() {
    let stream = stream?;
    std::thread::spawn(move || {
        let server = match WebSocketServer::bind(stream, "/ws") {
            Ok(server) => server,
            Err(e) => {
                eprintln!("handshake failed: {e}");
                return;
            }
        };

        let config = WebSocketConfig {
            ping_interval: Some(Duration::from_secs(30)),
        };

        let result = server.accept_with_callback_and_config(
            |writer, event| match event {
                ServerEvent::Connected => { /* writer is ready */ }
                ServerEvent::Frame(frame) => {
                    let _ = writer.send(&frame); // echo
                }
                ServerEvent::Closed => { /* peer closed */ }
                ServerEvent::Error(e) => eprintln!("error: {e}"),
            },
            config,
        );

        if let Err(e) = result {
            eprintln!("connection error: {e}");
        }
    });
}
```

### ServerEvent

The callback receives `ServerEvent` variants:

| Variant | When | Description |
|---------|------|-------------|
| `Connected` | Before reader loop | Client completed the WebSocket handshake |
| `Frame(Frame)` | Each frame | A parsed WebSocket frame |
| `Closed` | Connection closed | The peer closed the connection |
| `Error(String)` | I/O or framing failure | Error message |

### WebSocketConfig

```rust
pub struct WebSocketConfig {
    pub ping_interval: Option<Duration>,
}
```

`WebSocketConfig` lives in `runact_web::websocket::async_ws` and implements
`Default`. When `ping_interval` is set, a background thread sends Ping frames
at the configured interval; the reader auto-responds to incoming Pings with
Pongs.

## 14.2 Stream Abstraction

`WebSocketServer` is generic over `S: Read + Write + SetReadTimeout + Send + 'static`.
This allows it to work with both standard TCP streams and runact's native
TCP streams:

### Standard TCP

```rust
let server = WebSocketServer::<std::net::TcpStream>::bind(stream, "/ws")?;
```

### Runact TCP Bridge

```rust
use runact_web::websocket::runact_tcp::RunactTcpStream;

let adapter = RunactTcpStream(runact_stream);
let server = WebSocketServer::<RunactTcpStream>::bind(adapter, "/ws")?;
```

The `RunactTcpStream` adapter is a tuple struct wrapping
`runact::net::tcp_api::TcpStream` and implements `Read`, `Write`, and
`SetReadTimeout` (no-op, since runact's TCP is non-blocking).

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
    pub fn send(&self, frame: &Frame) -> Result<(), SendError>;
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
use runact_web::websocket::async_ws::WebSocketConfig;
use runact_web::websocket::frame::OpCode;
use runact_web::websocket::server::{ServerEvent, WebSocketServer};
use std::time::Duration;

// `stream` is an accepted TcpStream (std or RunactTcpStream).
let server = WebSocketServer::bind(stream, "/ws")?;

server.accept_with_callback_and_config(
    |writer, event| {
        match event {
            ServerEvent::Connected => println!("Client connected"),
            ServerEvent::Frame(frame) => {
                // Echo back text and binary frames
                if matches!(frame.opcode, OpCode::Text | OpCode::Binary) {
                    let _ = writer.send(&frame);
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
