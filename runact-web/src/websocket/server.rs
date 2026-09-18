//! WebSocket server: handshake + connection lifecycle.
//!
//! [`WebSocketServer`] accepts a TCP stream, validates the WebSocket upgrade
//! request, sends the 101 Switching Protocols response, and wraps the
//! connection in an [`AsyncWebSocket`] for non-blocking frame
//! non-blocking frame I/O.
//!
//! ## Usage
//!
//! ```no_run
//! use runact_web::websocket::server::WebSocketServer;
//! use std::net::TcpStream;
//!
//! let stream: TcpStream = /* from accept() */;
//! let mut server = WebSocketServer::bind(stream, "/ws").unwrap();
//! server.accept_with_callback(|event| {
//!     match event {
//!         runact_web::websocket::server::ServerEvent::Frame(f) => {
//!             // echo text frames
//!         }
//!         _ => {}
//!     }
//! });
//! ```

use crate::request::Request;
use crate::websocket::async_ws::{AsyncWebSocket, AsyncWriter, Message, SendError};
use crate::websocket::frame::{Frame, OpCode};
use crate::websocket::upgrade::{UpgradeError, build_accept_key, validate_upgrade_request};
use std::io::{Read, Write};
use std::net::TcpStream as StdTcpStream;
use std::sync::mpsc;

/// Events delivered to the server callback.
#[derive(Debug)]
pub enum ServerEvent {
    /// The connection has been established and the writer is ready.
    Connected,
    /// A parsed WebSocket frame.
    Frame(Frame),
    /// The peer closed the connection cleanly.
    Closed,
    /// An I/O or framing error.
    Error(String),
}

impl From<Message> for ServerEvent {
    fn from(msg: Message) -> Self {
        match msg {
            Message::Frame(f) => ServerEvent::Frame(f),
            Message::Closed => ServerEvent::Closed,
            Message::Error(e) => ServerEvent::Error(e.to_string()),
        }
    }
}

/// A connection-oriented handle for sending frames back to a client.
#[derive(Debug, Clone)]
pub struct ConnectionWriter(AsyncWriter);

impl ConnectionWriter {
    /// Send a frame to the peer. Non-blocking.
    pub fn send(&self, frame: &Frame) -> Result<(), SendError> {
        self.0.send(frame)
    }

    /// Send a text frame to the peer.
    pub fn send_text(&self, text: &str) -> Result<(), SendError> {
        self.0.send(&Frame {
            fin: true,
            opcode: OpCode::Text,
            masked: false,
            mask_key: [0; 4],
            payload: text.as_bytes().to_vec(),
        })
    }

    /// Send a binary frame to the peer.
    pub fn send_binary(&self, data: &[u8]) -> Result<(), SendError> {
        self.0.send(&Frame {
            fin: true,
            opcode: OpCode::Binary,
            masked: false,
            mask_key: [0; 4],
            payload: data.to_vec(),
        })
    }

    /// Send a close frame with the given status code.
    pub fn send_close(&self, status: u16) -> Result<(), SendError> {
        self.0.send_close(status)
    }

    /// Send a ping frame with the given payload.
    pub fn send_ping(&self, payload: &[u8]) -> Result<(), SendError> {
        self.0.send_ping(payload)
    }

    /// Send a pong frame with the given payload.
    pub fn send_pong(&self, payload: &[u8]) -> Result<(), SendError> {
        self.0.send_pong(payload)
    }
}

impl From<AsyncWriter> for ConnectionWriter {
    fn from(writer: AsyncWriter) -> Self {
        ConnectionWriter(writer)
    }
}

/// State held by the server.
struct PendingHandshake {
    stream: StdTcpStream,
    request: Request,
}

/// WebSocket server connection.
///
/// Created via [`WebSocketServer::bind`], which reads and validates the HTTP
/// upgrade request. After [`accept`](Self::accept) sends the 101 response,
/// the server wraps the connection in an [`AsyncWebSocket`] for non-blocking
/// frame I/O with a caller-provided callback.
pub struct WebSocketServer {
    pending: Option<PendingHandshake>,
}

impl std::fmt::Debug for WebSocketServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketServer")
            .field("has_pending", &self.pending.is_some())
            .finish()
    }
}

impl WebSocketServer {
    /// Bind a `WebSocketServer` to a TCP stream for the given path.
    ///
    /// Reads the HTTP upgrade request and validates the upgrade headers.
    /// Does NOT send the 101 response — call [`accept`](Self::accept) or
    /// [`accept_with_callback`](Self::accept_with_callback) next.
    pub fn bind(stream: StdTcpStream, path: &str) -> Result<Self, HandshakeError> {
        let mut stream = stream;
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(100)))
            .map_err(|e| HandshakeError::Io(e.to_string()))?;

        let mut buf = [0u8; 4096];
        let n = stream
            .read(&mut buf)
            .map_err(|e| HandshakeError::Io(e.to_string()))?;
        let request_str = std::str::from_utf8(&buf[..n])
            .map_err(|e| HandshakeError::InvalidUtf8(e.to_string()))?;

        let req = Request::parse(request_str.as_bytes()).map_err(HandshakeError::ParseError)?;

        // Validate the path matches
        if req.path != path {
            return Err(HandshakeError::PathMismatch {
                expected: path.to_string(),
                got: req.path,
            });
        }

        // Validate upgrade headers before proceeding
        validate_upgrade_request(&req.headers).map_err(HandshakeError::UpgradeError)?;

        Ok(Self {
            pending: Some(PendingHandshake {
                stream,
                request: req,
            }),
        })
    }

    /// Complete the WebSocket handshake and return the [`AsyncWebSocket`].
    ///
    /// Sends the 101 Switching Protocols response with the correct
    /// `Sec-WebSocket-Accept` header, then wraps the stream in an
    /// [`AsyncWebSocket`] with a no-op reader callback.
    ///
    /// To receive frames, use [`accept_with_callback`](Self::accept_with_callback)
    /// instead, or manually wrap the returned [`AsyncWebSocket`].
    pub fn accept(&mut self) -> Result<AsyncWebSocket, HandshakeError> {
        let mut pending = self
            .pending
            .take()
            .ok_or(HandshakeError::HandshakeAlreadyPerformed)?;

        let key = pending.request.headers.get("Sec-WebSocket-Key").ok_or(
            HandshakeError::UpgradeError(UpgradeError::MissingHeader("Sec-WebSocket-Key".into())),
        )?;

        let accept = build_accept_key(key);

        // Build and send the 101 response
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {accept}\r\n\
             \r\n"
        );
        pending
            .stream
            .write_all(response.as_bytes())
            .map_err(|e| HandshakeError::Io(e.to_string()))?;
        pending
            .stream
            .flush()
            .map_err(|e| HandshakeError::Io(e.to_string()))?;

        // Wrap the stream in AsyncWebSocket with a no-op callback
        Ok(AsyncWebSocket::with_callback(pending.stream, |_| {}))
    }

    /// Complete the WebSocket handshake and start the reader loop.
    ///
    /// Sends the 101 response, then blocks until the connection closes or an
    /// error occurs. The callback receives a [`ConnectionWriter`] (for sending
    /// frames back to the peer) and a [`ServerEvent`] (incoming frame, close,
    /// or error).
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use runact_web::websocket::server::{WebSocketServer, ServerEvent};
    /// # use runact_web::websocket::frame::OpCode;
    /// # let stream: std::net::TcpStream = /* ... */;
    /// # let mut server = WebSocketServer::bind(stream, "/ws").unwrap();
    /// server.accept_with_callback(|writer, event| {
    ///     match event {
    ///         ServerEvent::Frame(f) if f.opcode == OpCode::Text => {
    ///             let _ = writer.send(&f); // echo
    ///         }
    ///         ServerEvent::Frame(f) if f.opcode == OpCode::Close => {
    ///             let _ = writer.send_close(1000);
    ///         }
    ///         _ => {}
    ///     }
    /// }).expect("handshake");
    /// ```
    pub fn accept_with_callback<F>(mut self, callback: F) -> Result<(), HandshakeError>
    where
        F: Fn(&ConnectionWriter, ServerEvent) + Send + Sync + 'static,
    {
        let mut pending = self
            .pending
            .take()
            .ok_or(HandshakeError::HandshakeAlreadyPerformed)?;

        let key = pending.request.headers.get("Sec-WebSocket-Key").ok_or(
            HandshakeError::UpgradeError(UpgradeError::MissingHeader("Sec-WebSocket-Key".into())),
        )?;

        let accept = build_accept_key(key);

        // Build and send the 101 response
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {accept}\r\n\
             \r\n"
        );
        pending.stream.write_all(response.as_bytes())?;
        pending.stream.flush()?;

        // Create AsyncWebSocket with a channel-based callback, then forward
        // ServerEvents to the user callback with access to the ConnectionWriter.
        let (event_tx, event_rx) = mpsc::sync_channel::<ServerEvent>(1000);

        let ws = AsyncWebSocket::with_callback(pending.stream, move |msg| {
            let _ = event_tx.send(msg.into());
        });
        let conn_writer = ConnectionWriter(ws.get_writer());

        // Forward events to the user callback
        for event in event_rx.iter() {
            callback(&conn_writer, event);
        }

        Ok(())
    }

    /// Complete the WebSocket handshake and start the reader loop with
    /// configuration.
    ///
    /// Like [`accept_with_callback`](Self::accept_with_callback) but accepts a
    /// [`super::async_ws::WebSocketConfig`] for options such as ping interval heartbeats.
    pub fn accept_with_callback_and_config<F>(
        mut self,
        callback: F,
        config: super::async_ws::WebSocketConfig,
    ) -> Result<(), HandshakeError>
    where
        F: Fn(&ConnectionWriter, ServerEvent) + Send + Sync + 'static,
    {
        let mut pending = self
            .pending
            .take()
            .ok_or(HandshakeError::HandshakeAlreadyPerformed)?;

        let key = pending.request.headers.get("Sec-WebSocket-Key").ok_or(
            HandshakeError::UpgradeError(UpgradeError::MissingHeader("Sec-WebSocket-Key".into())),
        )?;

        let accept = build_accept_key(key);

        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {accept}\r\n\
             \r\n"
        );
        pending.stream.write_all(response.as_bytes())?;
        pending.stream.flush()?;

        let (event_tx, event_rx) = mpsc::sync_channel::<ServerEvent>(1000);

        let ws = AsyncWebSocket::with_callback_and_config(
            pending.stream,
            move |msg| {
                let _ = event_tx.send(msg.into());
            },
            config,
        );
        let conn_writer = ConnectionWriter(ws.get_writer());

        // Send Connected event first so the callback can register the writer
        callback(&conn_writer, ServerEvent::Connected);

        for event in event_rx.iter() {
            callback(&conn_writer, event);
        }

        Ok(())
    }

    /// Returns the path this server is bound to.
    pub fn path(&self) -> &str {
        self.pending
            .as_ref()
            .map(|p| p.request.path.as_str())
            .unwrap_or("")
    }

    /// Access the HTTP upgrade request headers (before handshake).
    pub fn request_headers(&self) -> Option<&crate::headers::Headers> {
        self.pending.as_ref().map(|p| &p.request.headers)
    }
}

/// Errors during WebSocket handshake.
#[derive(Debug)]
pub enum HandshakeError {
    /// An I/O error reading the request or writing the response.
    Io(String),
    /// The request bytes were not valid UTF-8.
    InvalidUtf8(String),
    /// The HTTP request could not be parsed.
    ParseError(crate::headers::HttpError),
    /// The requested path does not match the server's path.
    PathMismatch { expected: String, got: String },
    /// The upgrade request failed validation.
    UpgradeError(UpgradeError),
    /// `accept()` or `accept_with_callback()` was called after the handshake
    /// was already performed.
    HandshakeAlreadyPerformed,
}

impl std::fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandshakeError::Io(e) => write!(f, "IO error: {e}"),
            HandshakeError::InvalidUtf8(e) => write!(f, "invalid UTF-8: {e}"),
            HandshakeError::ParseError(e) => write!(f, "parse error: {e}"),
            HandshakeError::PathMismatch { expected, got } => {
                write!(f, "path mismatch: expected {expected}, got {got}")
            }
            HandshakeError::UpgradeError(e) => write!(f, "upgrade error: {e}"),
            HandshakeError::HandshakeAlreadyPerformed => {
                write!(f, "handshake already performed")
            }
        }
    }
}

impl std::error::Error for HandshakeError {}

impl From<crate::headers::HttpError> for HandshakeError {
    fn from(e: crate::headers::HttpError) -> Self {
        HandshakeError::ParseError(e)
    }
}

impl From<std::io::Error> for HandshakeError {
    fn from(e: std::io::Error) -> Self {
        HandshakeError::Io(e.to_string())
    }
}
