use runact_web::websocket::async_ws::WebSocketConfig;
use runact_web::websocket::frame::{Frame, OpCode};
use runact_web::websocket::server::{ConnectionWriter, ServerEvent, WebSocketServer};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Test broadcast pattern: two clients connect, both receive messages
/// from the other client.
#[test]
fn test_chat_broadcast_two_clients() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    // Shared peer registry
    let peers: Arc<Mutex<Vec<ConnectionWriter>>> = Arc::new(Mutex::new(Vec::new()));

    // Server thread — accepts connections and runs the chat loop
    let server_peers = Arc::clone(&peers);
    let _server_thread = thread::spawn(move || {
        loop {
            let stream = match listener.accept() {
                Ok((s, _)) => s,
                Err(e) => {
                    eprintln!("accept error: {e}");
                    break;
                }
            };

            let peers = Arc::clone(&server_peers);
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
                    move |writer, event| {
                        // Register writer on Connected event
                        if let ServerEvent::Connected = event {
                            let mut peer_list = peers.lock().unwrap();
                            peer_list.push(writer.clone());
                        }

                        if let ServerEvent::Frame(f) = event {
                            if f.opcode == OpCode::Text {
                                let text = String::from_utf8_lossy(&f.payload).to_string();
                                let peer_list = peers.lock().unwrap();
                                for peer in peer_list.iter() {
                                    let _ = peer.send_text(&text);
                                }
                            }
                        }
                    },
                    config,
                ) {
                    eprintln!("handshake failed: {e}");
                }
            });
        }
    });

    // --- Client 2 (receiver) — connects first ---
    let addr2 = addr;
    let client2_thread = thread::spawn(move || {
        let mut client = std::net::TcpStream::connect(addr2).expect("connect");
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("set timeout");

        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let request = format!(
            "GET /chat HTTP/1.1\r\n\
             Host: 127.0.0.1\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {key}\r\n\
             Sec-WebSocket-Version: 13\r\n\
             \r\n"
        );
        client.write_all(request.as_bytes()).expect("write");
        client.flush().expect("flush");

        // Read 101 response
        let mut resp_buf = [0u8; 4096];
        let n = client.read(&mut resp_buf).expect("read response");
        let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
        assert!(resp_str.contains("101"));

        // Wait for the broadcast message from client1
        let mut buf = [0u8; 256];
        let n = client.read(&mut buf).expect("read broadcast");
        let frame = Frame::parse(&buf[..n]).expect("parse broadcast");
        assert_eq!(frame.opcode, OpCode::Text);
        assert_eq!(
            std::str::from_utf8(&frame.payload).unwrap(),
            "hello from client1"
        );

        client
    });

    // Give client2 time to connect and register
    thread::sleep(Duration::from_millis(500));

    // --- Client 1 (sender) — connects and sends a message ---
    {
        let mut client = std::net::TcpStream::connect(addr).expect("connect");
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("set timeout");

        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let request = format!(
            "GET /chat HTTP/1.1\r\n\
             Host: 127.0.0.1\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {key}\r\n\
             Sec-WebSocket-Version: 13\r\n\
             \r\n"
        );
        client.write_all(request.as_bytes()).expect("write");
        client.flush().expect("flush");

        // Read 101 response
        let mut resp_buf = [0u8; 4096];
        let n = client.read(&mut resp_buf).expect("read response");
        let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
        assert!(resp_str.contains("101"));

        // Send a text frame
        let frame = Frame {
            fin: true,
            opcode: OpCode::Text,
            masked: false,
            mask_key: [0; 4],
            payload: b"hello from client1".to_vec(),
        };
        client.write_all(&frame.encode()).expect("write frame");
        client.flush().expect("flush");
    }

    // Wait for client2 to receive the broadcast
    match client2_thread.join() {
        Ok(_) => {}
        Err(e) => {
            eprintln!("client2 thread panicked: {:?}", e);
            panic!("client2 join: {:?}", e);
        }
    }

    // Clean up
    {
        let mut peer_list = peers.lock().unwrap();
        peer_list.clear();
    }
}
