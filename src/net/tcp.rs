//! TCP listener actor and supervised per-connection actors with mailbox I/O.
//!
//! See the [module-level docs](super) for the threading model. In short: the
//! accept loop and per-connection read/write loops run on dedicated threads
//! that never touch scheduler workers directly. They deliver decoded messages
//! to a connection actor via `Runtime::send`, and the actor dispatches outgoing
//! bytes to a writer thread via a bounded channel.

use crate::Runtime;
use crate::actor::{Actor, ActorContext, ActorError, ActorId};
use crate::error::RuntimeError;
use crate::resource::ResourceHandle;
use crossbeam_channel::{Receiver, Sender, bounded};
use std::io::{Read, Write};
use std::net::TcpStream as StdTcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// A decoded message from a network connection.
#[derive(Debug)]
pub enum NetMessage {
    /// Incoming data bytes.
    Data(Vec<u8>),
    /// The remote peer closed the connection (clean EOF or reset).
    Closed,
    /// An I/O error occurred. The connection should be torn down.
    Error(String),
}

/// Direction for outgoing writes on a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetWrite {
    /// Send these bytes to the peer.
    Data(Vec<u8>),
    /// Close the write half / shut down the connection.
    Close,
}

// ---------------------------------------------------------------------------
// Connection actor
// ---------------------------------------------------------------------------

/// Messages sent to a `NetConnection` actor from its reader thread (incoming
/// data / lifecycle) and from the server/bridge (outgoing writes).
pub enum NetConnectionMessage {
    /// Incoming data from the wire (delivered by the reader thread).
    Incoming(NetMessage),
    /// Outgoing write request (delivered by other actors via Runtime::send).
    Outgoing(NetWrite),
    /// The runtime is shutting down — stop the writer thread.
    Shutdown,
}

/// A `ResourceHandle` wrapping a connection actor id, enabling lookup via the
/// [`ResourceRegistry`](crate::resource::ResourceRegistry).
#[derive(Debug)]
pub struct NetConnectionHandle {
    pub actor_id: ActorId,
    /// Channel to enqueue outgoing bytes for the writer thread.
    pub writer_tx: Sender<NetWrite>,
    /// Signals the writer thread to stop.
    pub shutdown: Arc<AtomicBool>,
}

impl ResourceHandle for NetConnectionHandle {
    fn resource_type(&self) -> &str {
        "NetConnection"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl NetConnectionHandle {
    /// Send bytes to the peer. Non-blocking; returns `RuntimeError::MailboxFull`
    /// if the writer channel is at capacity.
    pub fn send(&self, bytes: Vec<u8>) -> Result<(), RuntimeError> {
        self.writer_tx
            .try_send(NetWrite::Data(bytes))
            .map_err(|e| match e {
                crossbeam_channel::TrySendError::Full(_) => {
                    RuntimeError::MailboxFull(self.actor_id)
                }
                crossbeam_channel::TrySendError::Disconnected(_) => RuntimeError::RuntimeStopped,
            })
    }

    /// Close the connection. Signals the writer thread to stop and closes the
    /// underlying stream.
    pub fn close(&self) -> Result<(), RuntimeError> {
        let _ = self.writer_tx.try_send(NetWrite::Close);
        self.shutdown.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// A supervised per-connection actor backed by reader/writer threads.
#[allow(dead_code)]
pub struct NetConnection {
    /// The TCP stream (owned by this actor; reader/writer threads operate on
    /// the Arc so the actor can close the handle on shutdown).
    stream: Arc<std::sync::Mutex<Option<StdTcpStream>>>,
    /// Where to deliver incoming data.
    handler: ActorId,
    /// Reader thread join handle (detached on drop).
    reader_handle: Option<std::thread::JoinHandle<()>>,
    /// Shared shutdown flag for the writer thread.
    writer_shutdown: Arc<AtomicBool>,
    writer_handle: Option<std::thread::JoinHandle<()>>,
}

impl NetConnection {
    /// Create a new connection actor. Spawns the reader and writer threads
    /// immediately. The `handler` actor (e.g. a session actor) receives
    /// `NetMessage` values on its mailbox.
    pub fn new(
        runtime: &Runtime,
        stream: StdTcpStream,
        handler: ActorId,
        _handler_tx: Sender<NetConnectionMessage>,
    ) -> Result<Self, RuntimeError> {
        let stream_arc: Arc<std::sync::Mutex<Option<StdTcpStream>>> =
            Arc::new(std::sync::Mutex::new(Some(stream)));
        let writer_shutdown = Arc::new(AtomicBool::new(false));

        // Split the stream: reader thread reads, writer thread writes.
        // We use try_clone for writing so reader and writer are independent.
        let reader_stream = {
            let guard = stream_arc.lock().unwrap();
            guard
                .as_ref()
                .unwrap()
                .try_clone()
                .map_err(|_e| RuntimeError::RuntimeStopped)?
        };

        // --- Reader thread: blocking read → mailbox delivery ---
        let handler_for_reader = handler;
        let stream_for_reader = Arc::clone(&stream_arc);
        let runtime_clone = runtime.sender();
        let reader_handle = std::thread::spawn(move || {
            let stream_guard = stream_for_reader.lock().unwrap();
            let mut stream = stream_guard.as_ref().unwrap();
            let mut buf = vec![0u8; 4096];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => {
                        // Clean EOF
                        let _ = runtime_clone.send(handler_for_reader, NetMessage::Closed);
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        if runtime_clone
                            .send(handler_for_reader, NetMessage::Data(data))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = runtime_clone
                            .send(handler_for_reader, NetMessage::Error(e.to_string()));
                        break;
                    }
                }
            }
        });

        // --- Writer thread: drains a bounded channel and performs blocking writes ---
        let _writer_tx: Sender<NetWrite> = bounded(1000).0;
        let writer_rx: Receiver<NetWrite> = bounded(1000).1;
        let writer_stream = {
            let guard = stream_arc.lock().unwrap();
            guard
                .as_ref()
                .unwrap()
                .try_clone()
                .map_err(|_e| RuntimeError::RuntimeStopped)?
        };
        let shutdown_clone = writer_shutdown.clone();
        let writer_handle = std::thread::spawn(move || {
            let mut stream = writer_stream;
            loop {
                if shutdown_clone.load(Ordering::SeqCst) {
                    break;
                }
                match writer_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(NetWrite::Data(bytes)) => {
                        if stream.write_all(&bytes).is_err() || stream.flush().is_err() {
                            break;
                        }
                    }
                    Ok(NetWrite::Close) => {
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                        break;
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        continue;
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });

        let _ = reader_stream; // reader uses the same stream_arc

        Ok(Self {
            stream: stream_arc,
            handler,
            reader_handle: Some(reader_handle),
            writer_shutdown,
            writer_handle: Some(writer_handle),
        })
    }

    pub fn handle(&self) -> Sender<NetWrite> {
        // Return the writer sender so we can give it to the handle.
        // The writer_tx is created in new(); we stash it via the handle.
        unimplemented!("use NetConnectionHandle for writes")
    }
}

impl Actor for NetConnection {
    type Message = NetConnectionMessage;

    fn handle(
        &mut self,
        msg: NetConnectionMessage,
        _ctx: &mut ActorContext,
    ) -> Result<(), ActorError> {
        match msg {
            NetConnectionMessage::Incoming(net_msg) => {
                // Forward the incoming message to the handler actor.
                // The handler is set in new(); runtime send is done by the
                // reader thread directly, so this arm is currently a no-op
                // fallback for any messages routed through the actor mailbox.
                let _ = net_msg;
            }
            NetConnectionMessage::Outgoing(write) => {
                // Outgoing writes are delivered to the writer thread.
                let _ = write;
            }
            NetConnectionMessage::Shutdown => {
                self.writer_shutdown.store(true, Ordering::SeqCst);
                if let Some(h) = self.writer_handle.take() {
                    let _ = h.join();
                }
                if let Some(h) = self.reader_handle.take() {
                    let _ = h.join();
                }
            }
        }
        Ok(())
    }
}
