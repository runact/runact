//! A WebSocket echo server example using runact-web.
//!
//! Listens on 127.0.0.1:8080, accepts WebSocket connections at `/ws`,
//! and echoes text/binary frames back to the client. Sends periodic ping
//! frames every 30 seconds to keep connections alive.
//!
//! Run:
//! ```sh
//! cargo run -p runact-web --example websocket_echo
//! ```
//!
//! Test with websocat:
//! ```sh
//! websocat ws://127.0.0.1:8080/ws
//! ```
//! Then type messages — they will be echoed back.

use runact_web::websocket::async_ws::WebSocketConfig;
use runact_web::websocket::frame::OpCode;
use runact_web::websocket::server::{ServerEvent, WebSocketServer};
use std::thread;
use std::time::Duration;

fn main() {
    let listener = std::net::TcpListener::bind("127.0.0.1:8080").expect("bind");
    println!("WebSocket echo server listening on 127.0.0.1:8080");
    println!("Connect at ws://127.0.0.1:8080/ws");
    println!("Ping interval: 30s for connection keepalive");

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept error: {e}");
                continue;
            }
        };

        thread::spawn(move || {
            let server = match WebSocketServer::bind(stream, "/ws") {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("handshake error: {e}");
                    return;
                }
            };

            // Enable ping heartbeats every 30 seconds
            let config = WebSocketConfig {
                ping_interval: Some(Duration::from_secs(30)),
            };

            if let Err(e) = server.accept_with_callback_and_config(
                |writer, event| match event {
                    ServerEvent::Frame(f) => match f.opcode {
                        OpCode::Text | OpCode::Binary => {
                            let _ = writer.send(&f); // echo back
                        }
                        OpCode::Ping => {
                            // Auto-pong already handled by reader loop,
                            // but we can also respond manually if needed
                        }
                        OpCode::Close => {
                            let status = if f.payload.len() >= 2 {
                                u16::from_be_bytes([f.payload[0], f.payload[1]])
                            } else {
                                1000
                            };
                            let _ = writer.send_close(status);
                        }
                        _ => {}
                    },
                    ServerEvent::Closed => {
                        println!("client disconnected");
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
