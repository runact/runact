use runact_web::headers::Headers;
use runact_web::request::{Method, Request};
use runact_web::response::{Response, StatusCode};
use runact_web::router::Router;
use runact_web::websocket::frame::{Frame, OpCode};
use runact_web::websocket::server::{HandshakeError, ServerEvent, WebSocketServer};
use std::io::{Read, Write};
use std::net::TcpListener as StdTcpListener;
use std::net::TcpStream as StdTcpStream;
use std::thread;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Unit tests: handshake validation
// ---------------------------------------------------------------------------

#[test]
fn test_websocket_server_rejects_invalid_upgrade() {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    let client_thread = thread::spawn(move || {
        let mut client = StdTcpStream::connect(addr).expect("connect");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set timeout");

        // Send upgrade WITHOUT websocket headers
        let request = "GET /ws HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        client.write_all(request.as_bytes()).expect("write request");
        client.flush().expect("flush");
    });

    let (stream, _addr) = listener.accept().expect("accept");
    let result = WebSocketServer::bind(stream, "/ws");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        HandshakeError::UpgradeError(_)
    ));

    client_thread.join().expect("client thread");
}

#[test]
fn test_websocket_server_rejects_path_mismatch() {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    let client_thread = thread::spawn(move || {
        let mut client = StdTcpStream::connect(addr).expect("connect");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set timeout");

        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let request = format!(
            "GET /other HTTP/1.1\r\n\
             Host: 127.0.0.1\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {key}\r\n\
             Sec-WebSocket-Version: 13\r\n\
             \r\n"
        );
        client.write_all(request.as_bytes()).expect("write");
        client.flush().expect("flush");
    });

    let (stream, _addr) = listener.accept().expect("accept");
    let result = WebSocketServer::bind(stream, "/ws");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        HandshakeError::PathMismatch { .. }
    ));

    client_thread.join().expect("client thread");
}

// ---------------------------------------------------------------------------
// Full TCP roundtrip: handshake → echo → close
// ---------------------------------------------------------------------------

#[test]
fn test_websocket_server_full_roundtrip() {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    let client_thread = thread::spawn(move || {
        let mut client = StdTcpStream::connect(addr).expect("connect");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set timeout");

        // Send upgrade
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let request = format!(
            "GET /ws HTTP/1.1\r\n\
             Host: 127.0.0.1\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {key}\r\n\
             Sec-WebSocket-Version: 13\r\n\
             \r\n"
        );
        client.write_all(request.as_bytes()).expect("write request");
        client.flush().expect("flush");

        // Read 101 response
        let mut resp_buf = [0u8; 4096];
        let n = client.read(&mut resp_buf).expect("read upgrade response");
        let resp = Response::parse(&resp_buf[..n]).expect("parse response");
        assert_eq!(resp.status, StatusCode::SwitchingProtocols);
        assert_eq!(resp.headers.get("Upgrade").unwrap(), "websocket");
        assert_eq!(
            resp.headers.get("Sec-WebSocket-Accept").unwrap(),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );

        // Send a text frame
        let frame = Frame {
            fin: true,
            opcode: OpCode::Text,
            masked: false,
            mask_key: [0; 4],
            payload: b"hello echo".to_vec(),
        };
        client.write_all(&frame.encode()).expect("write frame");
        client.flush().expect("flush");

        // Read echoed text frame
        let mut echo_buf = [0u8; 1024];
        let n = client.read(&mut echo_buf).expect("read echo");
        let echoed = Frame::parse(&echo_buf[..n]).expect("parse echo");
        assert_eq!(echoed.opcode, OpCode::Text);
        assert_eq!(std::str::from_utf8(&echoed.payload).unwrap(), "hello echo");

        // Send a close frame and read the close echo
        let close = Frame {
            fin: true,
            opcode: OpCode::Close,
            masked: false,
            mask_key: [0; 4],
            payload: 1000u16.to_be_bytes().to_vec(),
        };
        client.write_all(&close.encode()).expect("write close");
        client.flush().expect("flush");

        let n = client.read(&mut echo_buf).expect("read close echo");
        let close_resp = Frame::parse(&echo_buf[..n]).expect("parse close echo");
        assert_eq!(close_resp.opcode, OpCode::Close);
        assert_eq!(close_resp.payload, 1000u16.to_be_bytes().to_vec());
    });

    // Server side
    let (stream, _addr) = listener.accept().expect("accept");
    let server = WebSocketServer::bind(stream, "/ws").expect("bind");

    // Use accept_with_callback to echo text frames and close on close
    server
        .accept_with_callback(|writer, event| {
            if let ServerEvent::Frame(f) = event {
                match f.opcode {
                    OpCode::Text => {
                        let _ = writer.send(&f); // echo back
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
                }
            }
        })
        .expect("handshake");

    client_thread.join().expect("client thread");
}

// ---------------------------------------------------------------------------
// Router integration: WebSocket upgrade route returns 101
// ---------------------------------------------------------------------------

#[test]
fn test_router_websocket_upgrade_handler() {
    let mut router = Router::new();
    router.route(Method::Get, "/ws", |_req: Request| -> Response {
        let mut headers = Headers::new();
        headers.insert("Upgrade", "websocket");
        headers.insert("Connection", "Upgrade");
        headers.insert("Sec-WebSocket-Accept", "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
        Response {
            status: StatusCode::SwitchingProtocols,
            reason: "Switching Protocols".to_string(),
            version: "HTTP/1.1".to_string(),
            headers,
            body: vec![],
        }
    });

    let mut req = Request::new(Method::Get, "/ws", "HTTP/1.1");
    req.headers_mut().insert("Upgrade", "websocket");
    req.headers_mut().insert("Connection", "Upgrade");
    req.headers_mut()
        .insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==");
    req.headers_mut().insert("Sec-WebSocket-Version", "13");

    let resp = router.handle(req);
    assert_eq!(resp.status, StatusCode::SwitchingProtocols);
    assert_eq!(resp.headers.get("Upgrade").unwrap(), "websocket");
}
