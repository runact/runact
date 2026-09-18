use runact_web::headers::Headers;
use runact_web::request::{Method, Request};
use runact_web::response::{Response, StatusCode};
use runact_web::router::Router;
use runact_web::websocket::async_ws::{AsyncWebSocket, Message};
use runact_web::websocket::frame::{Frame, OpCode};
use runact_web::websocket::upgrade::validate_upgrade_request;
use std::io::{Read, Write};
use std::net::TcpListener as StdTcpListener;
use std::net::TcpStream as StdTcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// ---------------------------------------------------------------------------
// DuplexStream: in-memory Read+Write for unit tests
// ---------------------------------------------------------------------------

struct DuplexStream {
    read_buf: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    write_buf: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl DuplexStream {
    fn pair() -> (Self, Self) {
        let a = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let b = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        (
            DuplexStream {
                read_buf: a.clone(),
                write_buf: b.clone(),
            },
            DuplexStream {
                read_buf: b,
                write_buf: a,
            },
        )
    }
}

impl Read for DuplexStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut guard = self.read_buf.lock().unwrap();
        if guard.is_empty() {
            return Err(std::io::ErrorKind::WouldBlock.into());
        }
        let n = guard.len().min(buf.len());
        buf[..n].copy_from_slice(&guard[..n]);
        guard.drain(..n);
        Ok(n)
    }
}

impl Write for DuplexStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.write_buf.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AsyncWebSocket unit tests (in-memory)
// ---------------------------------------------------------------------------

#[test]
fn test_async_websocket_send_frame() {
    let (mut client, server) = DuplexStream::pair();

    let _tx = mpsc::channel::<()>();
    let ws = AsyncWebSocket::with_callback(server, |_| {});
    let writer = ws.get_writer();

    // Send a text frame
    writer
        .send(&Frame {
            fin: true,
            opcode: OpCode::Text,
            masked: false,
            mask_key: [0; 4],
            payload: b"Hello async".to_vec(),
        })
        .expect("send frame");

    // Small delay for the writer thread to flush
    thread::sleep(Duration::from_millis(50));

    let mut buf = Vec::new();
    let _ = client.read_to_end(&mut buf);
    assert!(!buf.is_empty());
    let frame = Frame::parse(&buf).expect("parse frame");
    assert_eq!(frame.opcode, OpCode::Text);
    assert_eq!(std::str::from_utf8(&frame.payload).unwrap(), "Hello async");
    let _ = client;
}

#[test]
fn test_async_websocket_read_frame() {
    let (mut client, server) = DuplexStream::pair();

    // Write a text frame into the server's read side (via client's write side)
    let frame = Frame {
        fin: true,
        opcode: OpCode::Text,
        masked: false,
        mask_key: [0; 4],
        payload: b"ping".to_vec(),
    };
    client.write_all(&frame.encode()).expect("write");

    let (tx, rx) = mpsc::channel();
    let ws = AsyncWebSocket::with_callback(server, move |msg| {
        tx.send(msg).expect("send to test");
    });
    let _ = ws;

    let msg = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("receive message");
    match msg {
        Message::Frame(f) => {
            assert_eq!(f.opcode, OpCode::Text);
            assert_eq!(std::str::from_utf8(&f.payload).unwrap(), "ping");
        }
        Message::Closed => panic!("expected frame, got closed"),
        Message::Error(e) => panic!("expected frame, got error: {e}"),
    }
}

#[test]
fn test_async_websocket_send_close() {
    let (mut client, server) = DuplexStream::pair();

    let _tx = mpsc::channel::<()>();
    let ws = AsyncWebSocket::with_callback(server, |_| {});
    let writer = ws.get_writer();

    writer.send_close(1000).expect("send close");
    thread::sleep(Duration::from_millis(50));

    let mut buf = Vec::new();
    let _ = client.read_to_end(&mut buf);
    assert!(!buf.is_empty());
    let frame = Frame::parse(&buf).expect("parse close frame");
    assert_eq!(frame.opcode, OpCode::Close);
    assert_eq!(frame.payload, 1000u16.to_be_bytes().to_vec());
}

// ---------------------------------------------------------------------------
// AsyncWebSocket: full request-response roundtrip on TCP
// ---------------------------------------------------------------------------

#[test]
fn test_async_websocket_full_roundtrip() {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    // Client side: connect, upgrade, send frame, read echo, close
    let client_addr = addr;
    let client_thread = thread::spawn(move || {
        let mut client = StdTcpStream::connect(client_addr).expect("connect");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set timeout");

        // Send upgrade request
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

        // Read the 101 response
        let mut resp_buf = [0u8; 4096];
        let n = client.read(&mut resp_buf).expect("read upgrade response");
        let resp_str = std::str::from_utf8(&resp_buf[..n]).expect("utf8");
        let resp = Response::parse(resp_str.as_bytes()).expect("parse response");
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
            payload: b"hello async ws".to_vec(),
        };
        client.write_all(&frame.encode()).expect("write frame");
        client.flush().expect("flush");

        // Read the echoed text frame
        let mut echo_buf = [0u8; 1024];
        let n = client.read(&mut echo_buf).expect("read echo");
        let echoed = Frame::parse(&echo_buf[..n]).expect("parse echo");
        assert_eq!(echoed.opcode, OpCode::Text);
        assert_eq!(
            std::str::from_utf8(&echoed.payload).unwrap(),
            "hello async ws"
        );

        // Send a close frame and read the server's close response
        let close = Frame {
            fin: true,
            opcode: OpCode::Close,
            masked: false,
            mask_key: [0; 4],
            payload: 1000u16.to_be_bytes().to_vec(),
        };
        client.write_all(&close.encode()).expect("write close");
        client.flush().expect("flush");

        let mut close_buf = [0u8; 1024];
        let n = client.read(&mut close_buf).expect("read close echo");
        let close_resp = Frame::parse(&close_buf[..n]).expect("parse close echo");
        assert_eq!(close_resp.opcode, OpCode::Close);
        assert_eq!(close_resp.payload, 1000u16.to_be_bytes().to_vec());
    });

    // Server side: accept, handshake, echo loop
    let (mut stream, _addr) = listener.accept().expect("accept");
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("set timeout");

    // Read the upgrade request
    let mut req_buf = [0u8; 4096];
    let n = stream.read(&mut req_buf).expect("read request");
    let request_str = std::str::from_utf8(&req_buf[..n]).expect("utf8");
    let req = Request::parse(request_str.as_bytes()).expect("parse request");
    validate_upgrade_request(&req.headers).expect("valid upgrade");

    let key = req.headers.get("Sec-WebSocket-Key").expect("key header");
    let accept = runact_web::websocket::upgrade::build_accept_key(key);

    // Send 101 response
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\
         \r\n"
    );
    stream
        .write_all(response.as_bytes())
        .expect("write response");
    stream.flush().expect("flush");

    // Create async WebSocket for the connection
    let (tx, rx) = mpsc::channel();
    let ws = AsyncWebSocket::with_callback(stream.try_clone().expect("clone"), move |msg| {
        let _ = tx.send(msg);
    });
    let writer = ws.get_writer();

    // Receive frames and echo text, handle close
    loop {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Message::Frame(f)) => match f.opcode {
                OpCode::Text => {
                    let reply = Frame {
                        fin: true,
                        opcode: OpCode::Text,
                        masked: false,
                        mask_key: [0; 4],
                        payload: f.payload.clone(),
                    };
                    writer.send(&reply).expect("send echo");
                }
                OpCode::Close => {
                    let status = if f.payload.len() >= 2 {
                        u16::from_be_bytes([f.payload[0], f.payload[1]])
                    } else {
                        1000
                    };
                    writer.send_close(status).expect("send close");
                    break;
                }
                _ => {}
            },
            Ok(Message::Closed) | Ok(Message::Error(_)) | Err(_) => break,
        }
    }

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
