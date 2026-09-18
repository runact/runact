//! WebSocket chat server example.
//!
//! A multi-client broadcast chat server using `runact-web`'s WebSocket stack.
//! Each connection runs in its own thread. Incoming text messages are
//! broadcast to all connected peers.
//!
//! Architecture:
//! - A global `Vec<ConnectionWriter>` is shared via `Arc<Mutex<>>`.
//! - On connect (`ServerEvent::Connected`), the peer's writer is added to the
//!   peer list.
//! - On incoming text frames, the message is forwarded to all peers.
//! - Periodic ping frames (30s) keep connections alive.
//!
//! Run:
//! ```sh
//! cargo run -p runact-web --example websocket_chat
//! ```
//!
//! Test with websocat:
//! ```sh
//! websocat ws://127.0.0.1:8080/chat
//! ```

use runact_web::websocket::async_ws::WebSocketConfig;
use runact_web::websocket::frame::OpCode;
use runact_web::websocket::server::ConnectionWriter;
use runact_web::websocket::server::{ServerEvent, WebSocketServer};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Shared registry of connected peer writers.
type PeerRegistry = Arc<Mutex<Vec<ConnectionWriter>>>;

fn main() {
    let listener = std::net::TcpListener::bind("127.0.0.1:8080").expect("bind");
    println!("Chat server listening on 127.0.0.1:8080");
    println!("Connect at ws://127.0.0.1:8080/chat");

    let peers: PeerRegistry = Arc::new(Mutex::new(Vec::new()));

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept error: {e}");
                continue;
            }
        };

        let peers = Arc::clone(&peers);

        thread::spawn(move || {
            let server = match WebSocketServer::bind(stream, "/chat") {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("handshake error: {e}");
                    return;
                }
            };

            let config = WebSocketConfig {
                ping_interval: Some(Duration::from_secs(30)),
            };

            if let Err(e) = server.accept_with_callback_and_config(
                move |writer, event| match event {
                    ServerEvent::Connected => {
                        // Register peer writer on connect
                        let mut peer_list = peers.lock().unwrap();
                        peer_list.push(writer.clone());
                    }
                    ServerEvent::Frame(f) => {
                        if f.opcode == OpCode::Text {
                            let text = String::from_utf8_lossy(&f.payload).to_string();
                            // Broadcast to all peers
                            let peer_list = peers.lock().unwrap();
                            for peer in peer_list.iter() {
                                let _ = peer.send_text(&text);
                            }
                        }
                    }
                    ServerEvent::Closed => {
                        println!("client disconnected");
                        // Clean up dead peers from registry
                        let mut peer_list = peers.lock().unwrap();
                        peer_list.retain(|p| p.send_ping(b"").is_ok());
                    }
                    ServerEvent::Error(e) => {
                        eprintln!("connection error: {e}");
                    }
                },
                config,
            ) {
                eprintln!("handshake failed: {e}");
            }
        });
    }
}
