//! Networking primitives for runact.
//!
//! This module provides the I/O layer that actors communicate through. It is
//! deliberately minimal: a TCP listener actor that spawns supervised
//! per-connection actors, each with dedicated reader/writer threads that feed
//! the connection actor's mailbox. The actor's `handle` method never blocks on
//! socket I/O — it only processes decoded messages and sends outgoing bytes
//! to a writer thread via a bounded channel.
//!
//! ## Threading model
//!
//! The scheduler runs actor loops on shared work-stealing workers that poll
//! `recv_timeout(10ms)`. A blocking read or write inside `handle()` would
//! stall an entire worker lane. Therefore each `NetConnection` spawns two
//! helper threads:
//!
//! - **Reader thread**: blocks on `TcpStream::read`, then sends a `NetMessage`
//!   (Data / Closed / Error) to the connection actor's mailbox via
//!   `Runtime::send`. The actor is `Sync`+`Send` so a cross-thread `Runtime`
//!   reference works.
//! - **Writer thread**: drains a `crossbeam_channel::Sender<Vec<u8>>` and
//!   performs blocking `write_all` + `flush`. The actor never blocks on writes.
//!
//! ## Transport bindings
//!
//! The raw byte stream this module delivers is consumed by higher-level
//! protocol codecs:
//!
//! - `tcp` — `TcpServer` actor and `NetConnection` with mailbox I/O.
//! - `ws` — RFC 6455 WebSocket handshake and framing.
//! - `http` — HTTP/1.1 request parsing and static file serving.
//! - `server` — composes the above into a full HTTP+WebSocket server assembly.
//!
//! No external I/O dependencies are used beyond `std` and `crossbeam-channel`
//! (already a runact dependency).

pub mod reactor;
pub mod tcp;
pub mod tcp_api;

pub use reactor::{Event, Interest, Reactor, Readiness};
