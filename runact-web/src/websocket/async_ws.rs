//! Async WebSocket connection over a byte stream.
//!
//! [`AsyncWebSocket`] wraps a `Read + Write` stream (e.g. `std::net::TcpStream`
//! or `runact::net::tcp_api::TcpStream`) and provides asynchronous frame
//! reading/writing without blocking runact scheduler workers.
//!
//! ## Architecture
//!
//! Two dedicated threads handle I/O, mirroring the pattern from
//! [`runact::net::tcp`]:
//!
//! - **Reader thread**: blocks on reading bytes, parses RFC 6455 frames, and
//!   delivers each frame to a callback. This keeps the frame-parsing logic off
//!   the async worker pool.
//! - **Writer thread**: drains a bounded channel of outgoing frames and writes
//!   them to the stream. Callers invoke [`AsyncWriter::send`] /
//!   [`AsyncWriter::send_close`] which are non-blocking (backpressure via a
//!   bounded channel).
//!
//! ## Invariants (async-runtime.md §22)
//!
//! - Pending tasks consume no worker time — reader/writer threads handle I/O.
//! - CPU-heavy work never runs on async/actor workers.
//! - Actors never block a worker waiting for I/O — they send frames via the
//!   non-blocking writer channel.
//! - Cancellation is cooperative — threads check a shutdown flag.

use crate::websocket::frame::{Frame, FrameError, OpCode};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A decoded WebSocket message from the reader loop.
#[derive(Debug)]
pub enum Message {
    /// A parsed frame (text, binary, ping, etc.).
    Frame(Frame),
    /// The remote peer closed the connection.
    Closed,
    /// An I/O or framing error occurred.
    Error(FrameError),
}

/// Error returned when sending a frame fails.
#[derive(Debug)]
pub enum SendError {
    /// The writer channel is full (backpressure).
    Full,
    /// The connection has been closed.
    Closed,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Full => write!(f, "send channel full (backpressure)"),
            SendError::Closed => write!(f, "connection closed"),
        }
    }
}

impl std::error::Error for SendError {}

impl<T> From<TrySendError<T>> for SendError {
    fn from(e: TrySendError<T>) -> Self {
        match e {
            TrySendError::Full(_) => SendError::Full,
            TrySendError::Disconnected(_) => SendError::Closed,
        }
    }
}

/// Direction for outgoing writes.
#[derive(Debug)]
enum Outbound {
    Frame(Frame),
    Close(u16),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Shutdown,
}

/// Handle for sending frames to the peer. Each call is non-blocking.
///
/// Created by [`AsyncWebSocket::get_writer`].
#[derive(Debug, Clone)]
pub struct AsyncWriter {
    tx: mpsc::SyncSender<Outbound>,
}

impl AsyncWriter {
    /// Send a frame to the peer. Non-blocking.
    ///
    /// Returns `Err(SendError::Full)` if the writer channel is full
    /// (backpressure) or `Err(SendError::Closed)` if the receiver has been
    /// dropped (connection closed).
    pub fn send(&self, frame: &Frame) -> Result<(), SendError> {
        self.tx
            .try_send(Outbound::Frame(frame.clone()))
            .map_err(Into::into)
    }

    /// Send a close frame with the given status code. Non-blocking.
    pub fn send_close(&self, status: u16) -> Result<(), SendError> {
        self.tx
            .try_send(Outbound::Close(status))
            .map_err(Into::into)
    }

    /// Send a ping frame with the given payload. Non-blocking.
    pub fn send_ping(&self, payload: &[u8]) -> Result<(), SendError> {
        self.tx
            .try_send(Outbound::Ping(payload.to_vec()))
            .map_err(Into::into)
    }

    /// Send a pong frame with the given payload. Non-blocking.
    pub fn send_pong(&self, payload: &[u8]) -> Result<(), SendError> {
        self.tx
            .try_send(Outbound::Pong(payload.to_vec()))
            .map_err(Into::into)
    }

    /// Signal the writer thread to stop and close the connection.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(Outbound::Shutdown);
    }
}

/// Configuration for [`AsyncWebSocket`].
#[derive(Debug, Clone, Default)]
pub struct WebSocketConfig {
    /// Interval for sending ping frames to keep the connection alive.
    /// When set, an internal thread sends ping frames at this interval.
    pub ping_interval: Option<Duration>,
}

/// An asynchronous WebSocket connection over a byte stream.
///
/// Spawns a reader thread and a writer thread for non-blocking I/O. The
/// reader delivers [`Message`]s to a callback; the writer drains an outbound
/// channel. Both threads operate independently of runact's scheduler workers.
pub struct AsyncWebSocket {
    writer: AsyncWriter,
    shutdown: Arc<AtomicBool>,
    reader_handle: Option<thread::JoinHandle<()>>,
    writer_handle: Option<thread::JoinHandle<()>>,
    ping_handle: Option<thread::JoinHandle<()>>,
}

impl AsyncWebSocket {
    /// Create a new async WebSocket over the given stream.
    ///
    /// The stream is consumed and shared between a reader thread and a writer
    /// thread. Incoming frames are delivered to `callback`.
    pub fn with_callback<S, F>(stream: S, callback: F) -> Self
    where
        S: Read + Write + Send + 'static,
        F: Fn(Message) + Send + Sync + 'static,
    {
        Self::with_callback_and_config(stream, callback, WebSocketConfig::default())
    }

    /// Create a new async WebSocket with configuration.
    ///
    /// Like [`with_callback`](Self::with_callback) but accepts a
    /// [`WebSocketConfig`] for options such as ping interval heartbeats.
    pub fn with_callback_and_config<S, F>(stream: S, callback: F, config: WebSocketConfig) -> Self
    where
        S: Read + Write + Send + 'static,
        F: Fn(Message) + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::sync_channel::<Outbound>(1000);
        let shutdown = Arc::new(AtomicBool::new(false));
        let stream_arc = Arc::new(Mutex::new(Some(stream)));
        let writer = AsyncWriter { tx: tx.clone() };

        let reader_stream = Arc::clone(&stream_arc);
        let shutdown_for_reader = shutdown.clone();
        let callback = Arc::new(callback);
        let reader_handle = thread::spawn(move || {
            Self::reader_loop(reader_stream, &callback, shutdown_for_reader);
        });

        let shutdown_for_writer = shutdown.clone();
        let writer_handle = thread::spawn(move || {
            Self::writer_loop(stream_arc, rx, shutdown_for_writer);
        });

        // Optional ping heartbeat thread
        let ping_handle = config.ping_interval.map(|interval| {
            let shutdown_for_ping = shutdown.clone();
            thread::spawn(move || {
                loop {
                    if shutdown_for_ping.load(Ordering::SeqCst) {
                        break;
                    }
                    thread::sleep(interval);
                    if shutdown_for_ping.load(Ordering::SeqCst) {
                        break;
                    }
                    let _ = tx.try_send(Outbound::Ping(Vec::new()));
                }
            })
        });

        Self {
            writer,
            shutdown,
            reader_handle: Some(reader_handle),
            writer_handle: Some(writer_handle),
            ping_handle,
        }
    }

    /// Returns a handle for sending frames to the peer.
    pub fn get_writer(&self) -> AsyncWriter {
        self.writer.clone()
    }

    fn reader_loop<S, F>(
        stream_arc: Arc<Mutex<Option<S>>>,
        callback: &Arc<F>,
        shutdown: Arc<AtomicBool>,
    ) where
        S: Read,
        F: Fn(Message),
    {
        let mut buf = vec![0u8; 4096];
        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            let mut guard = match stream_arc.lock() {
                Ok(g) => g,
                Err(_) => break,
            };

            let stream = match guard.as_mut() {
                Some(s) => s,
                None => break,
            };

            let read_result = stream.read(&mut buf);
            drop(guard); // release lock before callback

            match read_result {
                Ok(0) => {
                    callback(Message::Closed);
                    break;
                }
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    match Frame::parse(&data) {
                        Ok(frame) => match frame.opcode {
                            OpCode::Ping => {
                                callback(Message::Frame(frame.clone()));
                                // Auto-respond with pong
                                callback(Message::Frame(Frame {
                                    fin: true,
                                    opcode: OpCode::Pong,
                                    masked: false,
                                    mask_key: [0; 4],
                                    payload: frame.payload.clone(),
                                }));
                            }
                            _ => callback(Message::Frame(frame)),
                        },
                        Err(e) => {
                            callback(Message::Error(e));
                            break;
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }
                Err(e) => {
                    callback(Message::Error(FrameError::Io(e)));
                    break;
                }
            }
        }
    }

    fn writer_loop<S>(
        stream_arc: Arc<Mutex<Option<S>>>,
        rx: mpsc::Receiver<Outbound>,
        shutdown: Arc<AtomicBool>,
    ) where
        S: Write,
    {
        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            match rx.recv_timeout(std::time::Duration::from_millis(50)) {
                Ok(Outbound::Frame(frame)) => {
                    let encoded = frame.encode();
                    let mut guard = match stream_arc.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    if let Some(stream) = guard.as_mut() {
                        let _ = stream.write_all(&encoded);
                        let _ = stream.flush();
                    }
                }
                Ok(Outbound::Close(status)) => {
                    let frame = Frame {
                        fin: true,
                        opcode: OpCode::Close,
                        masked: false,
                        mask_key: [0; 4],
                        payload: status.to_be_bytes().to_vec(),
                    };
                    let encoded = frame.encode();
                    let mut guard = match stream_arc.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    if let Some(stream) = guard.as_mut() {
                        let _ = stream.write_all(&encoded);
                        let _ = stream.flush();
                    }
                }
                Ok(Outbound::Shutdown) => break,
                Ok(Outbound::Ping(payload)) => {
                    let frame = Frame {
                        fin: true,
                        opcode: OpCode::Ping,
                        masked: false,
                        mask_key: [0; 4],
                        payload,
                    };
                    let encoded = frame.encode();
                    let mut guard = match stream_arc.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    if let Some(stream) = guard.as_mut() {
                        let _ = stream.write_all(&encoded);
                        let _ = stream.flush();
                    }
                }
                Ok(Outbound::Pong(payload)) => {
                    let frame = Frame {
                        fin: true,
                        opcode: OpCode::Pong,
                        masked: false,
                        mask_key: [0; 4],
                        payload,
                    };
                    let encoded = frame.encode();
                    let mut guard = match stream_arc.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    if let Some(stream) = guard.as_mut() {
                        let _ = stream.write_all(&encoded);
                        let _ = stream.flush();
                    }
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }
}

impl Drop for AsyncWebSocket {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.writer.tx.try_send(Outbound::Shutdown);

        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.writer_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.ping_handle.take() {
            let _ = handle.join();
        }
    }
}
