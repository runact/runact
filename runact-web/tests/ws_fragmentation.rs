use runact_web::websocket::async_ws::{AsyncWebSocket, Message};
use runact_web::websocket::frame::{Frame, OpCode};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

/// Test that WebSocket message fragmentation works correctly.
///
/// Per RFC 6455 §5.4: a message can be split across multiple frames.
/// First frame: FIN=false, opcode=Text
/// Middle frames: FIN=false, opcode=Continuation
/// Last frame: FIN=true, opcode=Continuation
/// The receiver should reassemble these into a single message and
/// deliver it as a single Message::Frame with the complete payload.
#[test]
fn test_fragmented_message_reassembly() {
    // Create a TCP listener for the server side
    let server_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = server_listener.local_addr().expect("local_addr");

    // Server thread: accept connection, perform handshake, read a fragmented
    // message, and verify reassembly
    let server_thread = thread::spawn(move || {
        let (stream, _) = server_listener.accept().expect("accept");
        let mut stream = stream;

        // Read HTTP upgrade request
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).expect("read request");

        // Send 101 response
        let response = b"HTTP/1.1 101 Switching Protocols\r\n\
                         Upgrade: websocket\r\n\
                         Connection: Upgrade\r\n\
                         Sec-WebSocket-Accept: s3pFLPl3J6XayuHPqw3Li1qHB+M=\r\n\
                         \r\n";
        stream.write_all(response).expect("write handshake");
        stream.flush().expect("flush");

        // Wrap in AsyncWebSocket for reading
        let (tx, rx) = std::sync::mpsc::channel();
        let _ws = AsyncWebSocket::with_callback(stream, move |msg| {
            let _ = tx.send(msg);
        });

        // Wait for the reassembled message
        let msg = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("recv message");
        match msg {
            Message::Frame(frame) => {
                assert_eq!(frame.opcode, OpCode::Text);
                assert_eq!(frame.payload, b"Hello, fragmented WebSocket world!");
            }
            _ => panic!("expected Frame message, got {msg:?}"),
        }
    });

    // Client: connect, handshake, send fragmented message
    thread::sleep(Duration::from_millis(50));

    let mut client = TcpStream::connect(addr).expect("connect");
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set timeout");

    // Perform handshake
    let request = b"GET / HTTP/1.1\r\n\
                    Host: 127.0.0.1\r\n\
                    Upgrade: websocket\r\n\
                    Connection: Upgrade\r\n\
                    Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                    Sec-WebSocket-Version: 13\r\n\
                    \r\n";
    client.write_all(request).expect("write request");
    client.flush().expect("flush");

    // Read 101 response
    let mut resp_buf = [0u8; 4096];
    let n = client.read(&mut resp_buf).expect("read response");
    assert!(String::from_utf8_lossy(&resp_buf[..n]).contains("101"));

    // Send a fragmented message: "Hello, fragmented WebSocket world!" split into 3 frames
    let full_msg = b"Hello, fragmented WebSocket world!";

    // First frame: FIN=false, opcode=Text, first 7 bytes
    let frame1 = Frame {
        fin: false,
        opcode: OpCode::Text,
        masked: false,
        mask_key: [0; 4],
        payload: full_msg[..7].to_vec(),
    };
    client.write_all(&frame1.encode()).expect("write frame1");

    // Second frame: FIN=false, opcode=Continuation, next 10 bytes
    let frame2 = Frame {
        fin: false,
        opcode: OpCode::Continuation,
        masked: false,
        mask_key: [0; 4],
        payload: full_msg[7..17].to_vec(),
    };
    client.write_all(&frame2.encode()).expect("write frame2");

    // Third frame: FIN=true, opcode=Continuation, remaining bytes
    let frame3 = Frame {
        fin: true,
        opcode: OpCode::Continuation,
        masked: false,
        mask_key: [0; 4],
        payload: full_msg[17..].to_vec(),
    };
    client.write_all(&frame3.encode()).expect("write frame3");
    client.flush().expect("flush");

    // Close the write side so the server's reader gets EOF
    let _ = client.shutdown(std::net::Shutdown::Both);

    server_thread.join().expect("server thread");
}

/// Test that control frames interleave correctly with fragmented data.
#[test]
fn test_fragmented_message_with_interleaved_ping() {
    let server_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = server_listener.local_addr().expect("local_addr");

    let server_thread = thread::spawn(move || {
        let (stream, _) = server_listener.accept().expect("accept");
        let mut stream = stream;

        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).expect("read request");

        let response = b"HTTP/1.1 101 Switching Protocols\r\n\
                         Upgrade: websocket\r\n\
                         Connection: Upgrade\r\n\
                         Sec-WebSocket-Accept: s3pFLPl3J6XayuHPqw3Li1qHB+M=\r\n\
                         \r\n";
        stream.write_all(response).expect("write handshake");
        stream.flush().expect("flush");

        let (tx, rx) = std::sync::mpsc::channel();
        let _ws = AsyncWebSocket::with_callback(stream, move |msg| {
            let _ = tx.send(msg);
        });

        // Collect all messages. Control frames (Ping/Pong) are delivered
        // immediately even when interleaved with fragmented data, so the
        // order is: Ping, Pong (auto-response), then reassembled Text.
        let mut got_text = false;
        let mut got_pong = false;
        for _ in 0..3 {
            let msg = rx
                .recv_timeout(Duration::from_secs(5))
                .expect("recv message");
            match msg {
                Message::Frame(frame) => {
                    if frame.opcode == OpCode::Pong && frame.payload == b"ping" {
                        got_pong = true;
                    }
                    if frame.opcode == OpCode::Text && frame.payload == b"abcINTERLEAVED" {
                        got_text = true;
                    }
                }
                _ => {}
            }
        }
        assert!(
            got_text,
            "should receive reassembled Text frame with interleaved ping"
        );
        assert!(
            got_pong,
            "should receive Pong auto-response for interleaved Ping"
        );

        // Signal shutdown so the reader thread stops (avoid blocking on JoinHandle)
        // The _ws Drop will set the shutdown flag
    });

    thread::sleep(Duration::from_millis(50));

    let mut client = TcpStream::connect(addr).expect("connect");
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set timeout");

    let request = b"GET / HTTP/1.1\r\n\
                    Host: 127.0.0.1\r\n\
                    Upgrade: websocket\r\n\
                    Connection: Upgrade\r\n\
                    Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                    Sec-WebSocket-Version: 13\r\n\
                    \r\n";
    client.write_all(request).expect("write request");
    client.flush().expect("flush");

    let mut resp_buf = [0u8; 4096];
    let n = client.read(&mut resp_buf).expect("read response");
    assert!(String::from_utf8_lossy(&resp_buf[..n]).contains("101"));

    // Fragmentation with interleaved Ping:
    // Frame 1: FIN=false, Text, "ab"
    // Frame 2: FIN=false, Ping, "ping"
    // Frame 3: FIN=false, Continuation, "c"
    // Frame 4: FIN=true, Continuation, "INTERLEAVED"
    let frame1 = Frame {
        fin: false,
        opcode: OpCode::Text,
        masked: false,
        mask_key: [0; 4],
        payload: b"ab".to_vec(),
    };
    client.write_all(&frame1.encode()).expect("write frame1");

    let frame2 = Frame {
        fin: true,
        opcode: OpCode::Ping,
        masked: false,
        mask_key: [0; 4],
        payload: b"ping".to_vec(),
    };
    client.write_all(&frame2.encode()).expect("write ping");

    let frame3 = Frame {
        fin: false,
        opcode: OpCode::Continuation,
        masked: false,
        mask_key: [0; 4],
        payload: b"c".to_vec(),
    };
    client.write_all(&frame3.encode()).expect("write frame3");

    let frame4 = Frame {
        fin: true,
        opcode: OpCode::Continuation,
        masked: false,
        mask_key: [0; 4],
        payload: b"INTERLEAVED".to_vec(),
    };
    client.write_all(&frame4.encode()).expect("write frame4");
    client.flush().expect("flush");

    // Close the write side so the server's reader gets EOF
    let _ = client.shutdown(std::net::Shutdown::Both);

    server_thread.join().expect("server thread");
}
