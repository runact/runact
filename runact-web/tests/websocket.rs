use runact_web::headers::Headers;
use runact_web::request::{Method, Request};
use runact_web::response::{Response, StatusCode};
use runact_web::router::Router;
use runact_web::websocket::frame::{Frame, OpCode};
use runact_web::websocket::upgrade::{build_accept_key, validate_upgrade_request};

// ---------------------------------------------------------------------------
// Frame parsing tests (RFC 6455)
// ---------------------------------------------------------------------------

#[test]
fn test_frame_parse_text_single_byte_masked() {
    // A single-frame unmasked text message "Hello":
    // 0x81 0x85 0x37 0xfa 0x21 0x3d 0x7f 0x9f 0x4d 0x51 0x58
    let data: &[u8] = &[
        0x81, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58,
    ];
    let frame = Frame::parse(data).unwrap();
    assert!(frame.fin);
    assert_eq!(frame.opcode, OpCode::Text);
    assert!(frame.masked);
    assert_eq!(frame.payload.len(), 5);
    assert_eq!(std::str::from_utf8(&frame.payload).unwrap(), "Hello");
}

#[test]
fn test_frame_parse_unmasked_pong() {
    // Unmasked pong with empty payload: 0x8A 0x00
    let data: &[u8] = &[0x8A, 0x00];
    let frame = Frame::parse(data).unwrap();
    assert!(frame.fin);
    assert_eq!(frame.opcode, OpCode::Pong);
    assert!(!frame.masked);
    assert!(frame.payload.is_empty());
}

#[test]
fn test_frame_parse_close_with_status() {
    // Masked close frame with status 1000:
    // 0x88 0x82 0x00 0x00 0x00 0x00 0x03 0xe8
    let data: &[u8] = &[0x88, 0x82, 0x00, 0x00, 0x00, 0x00, 0x03, 0xe8];
    let frame = Frame::parse(data).unwrap();
    assert!(frame.fin);
    assert_eq!(frame.opcode, OpCode::Close);
    assert!(frame.masked);
    assert_eq!(frame.payload.len(), 2);
    let code = u16::from_be_bytes([frame.payload[0], frame.payload[1]]);
    assert_eq!(code, 1000);
}

#[test]
fn test_frame_parse_ping_with_payload() {
    // Masked ping with 2-byte payload:
    let data: &[u8] = &[0x89, 0x82, 0x12, 0x34, 0x56, 0x78, 0xAB, 0xCD];
    let frame = Frame::parse(data).unwrap();
    assert!(frame.fin);
    assert_eq!(frame.opcode, OpCode::Ping);
    assert!(frame.masked);
    assert_eq!(frame.payload.len(), 2);
}

#[test]
fn test_frame_encode_unmasked_text() {
    let frame = Frame {
        fin: true,
        opcode: OpCode::Text,
        masked: false,
        mask_key: [0; 4],
        payload: b"Hello".to_vec(),
    };
    let encoded = frame.encode();
    assert_eq!(encoded[0], 0x81); // FIN + Text
    assert_eq!(encoded[1], 0x05); // length 5, unmasked
    assert_eq!(&encoded[2..], b"Hello");
}

#[test]
fn test_frame_encode_decode_roundtrip() {
    let original = Frame {
        fin: true,
        opcode: OpCode::Binary,
        masked: false,
        mask_key: [0; 4],
        payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };
    let encoded = original.encode();
    let decoded = Frame::parse(&encoded).unwrap();
    assert!(decoded.fin);
    assert_eq!(decoded.opcode, OpCode::Binary);
    assert_eq!(decoded.payload, vec![0xDE, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn test_frame_extended_payload_length() {
    // Build a frame with 126-byte payload (uses 16-bit extended length)
    let payload = vec![0x42u8; 126];
    let frame = Frame {
        fin: true,
        opcode: OpCode::Text,
        masked: false,
        mask_key: [0; 4],
        payload: payload.clone(),
    };
    let encoded = frame.encode();
    assert_eq!(encoded[0], 0x81);
    assert_eq!(encoded[1], 126); // indicates 16-bit extended length
    assert_eq!(encoded[2], 0); // high byte of 126
    assert_eq!(encoded[3], 126); // low byte of 126
    assert_eq!(&encoded[4..], payload.as_slice());
}

#[test]
fn test_frame_fin_false_continuation() {
    let data: &[u8] = &[
        0x01, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f, // FIN=0, Text, "Hello"
    ];
    let frame = Frame::parse(data).unwrap();
    assert!(!frame.fin);
    assert_eq!(frame.opcode, OpCode::Text);
    assert_eq!(std::str::from_utf8(&frame.payload).unwrap(), "Hello");
}

#[test]
fn test_frame_opcodes() {
    assert_eq!(OpCode::from_u8(0x0), Some(OpCode::Continuation));
    assert_eq!(OpCode::from_u8(0x1), Some(OpCode::Text));
    assert_eq!(OpCode::from_u8(0x2), Some(OpCode::Binary));
    assert_eq!(OpCode::from_u8(0x8), Some(OpCode::Close));
    assert_eq!(OpCode::from_u8(0x9), Some(OpCode::Ping));
    assert_eq!(OpCode::from_u8(0xA), Some(OpCode::Pong));
    assert_eq!(OpCode::from_u8(0x5), None); // reserved
}

// ---------------------------------------------------------------------------
// Upgrade validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_upgrade_request_valid() {
    let mut headers = Headers::new();
    headers.insert("Upgrade", "websocket");
    headers.insert("Connection", "Upgrade");
    headers.insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==");
    headers.insert("Sec-WebSocket-Version", "13");

    assert!(validate_upgrade_request(&headers).is_ok());
}

#[test]
fn test_validate_upgrade_missing_upgrade_header() {
    let mut headers = Headers::new();
    headers.insert("Connection", "Upgrade");
    headers.insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==");
    headers.insert("Sec-WebSocket-Version", "13");

    assert!(validate_upgrade_request(&headers).is_err());
}

#[test]
fn test_validate_upgrade_wrong_version() {
    let mut headers = Headers::new();
    headers.insert("Upgrade", "websocket");
    headers.insert("Connection", "Upgrade");
    headers.insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==");
    headers.insert("Sec-WebSocket-Version", "12");

    assert!(validate_upgrade_request(&headers).is_err());
}

#[test]
fn test_validate_upgrade_missing_key() {
    let mut headers = Headers::new();
    headers.insert("Upgrade", "websocket");
    headers.insert("Connection", "Upgrade");
    headers.insert("Sec-WebSocket-Version", "13");

    assert!(validate_upgrade_request(&headers).is_err());
}

#[test]
fn test_build_accept_key() {
    // RFC 6455 example: key "dGhlIHNhbXBsZSBub25jZQ=="
    // Accept: "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    let accept = build_accept_key("dGhlIHNhbXBsZSBub25jZQ==");
    assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}

#[test]
fn test_upgrade_response_status() {
    let mut headers = Headers::new();
    headers.insert("Upgrade", "websocket");
    headers.insert("Connection", "Upgrade");
    headers.insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==");
    headers.insert("Sec-WebSocket-Version", "13");

    let resp = runact_web::websocket::upgrade::build_upgrade_response(&headers).unwrap();
    assert_eq!(resp.status, StatusCode::SwitchingProtocols);
    assert_eq!(resp.reason, "Switching Protocols");
    assert_eq!(resp.headers.get("Upgrade").unwrap(), "websocket");
    assert_eq!(resp.headers.get("Connection").unwrap(), "Upgrade");
    assert_eq!(
        resp.headers.get("Sec-WebSocket-Accept").unwrap(),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

#[test]
fn test_upgrade_response_includes_required_headers() {
    let mut headers = Headers::new();
    headers.insert("Upgrade", "websocket");
    headers.insert("Connection", "Upgrade");
    headers.insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==");
    headers.insert("Sec-WebSocket-Version", "13");

    let resp = runact_web::websocket::upgrade::build_upgrade_response(&headers).unwrap();
    // Must have Upgrade, Connection, and Sec-WebSocket-Accept
    assert!(resp.headers.get("Upgrade").is_some());
    assert!(resp.headers.get("Connection").is_some());
    assert!(resp.headers.get("Sec-WebSocket-Accept").is_some());
}

// ---------------------------------------------------------------------------
// WebSocket connection tests (in-process framing over a vec buffer)
// ---------------------------------------------------------------------------

#[test]
fn test_ws_connection_send_receive_text() {
    use runact_web::websocket::connection::WebSocketConnection;
    use std::io::Cursor;

    // Build a server-side connection over an in-memory buffer
    let client_frame = Frame {
        fin: true,
        opcode: OpCode::Text,
        masked: true,
        mask_key: [0x12, 0x34, 0x56, 0x78],
        payload: b"Hello, WebSocket!".to_vec(),
    };
    let mut buf = Vec::new();
    buf.extend_from_slice(&client_frame.encode());

    let mut conn = WebSocketConnection::new(Cursor::new(buf));
    let msg = conn.read_frame().unwrap();
    assert_eq!(msg.opcode, OpCode::Text);
    assert_eq!(
        std::str::from_utf8(&msg.payload).unwrap(),
        "Hello, WebSocket!"
    );
}

#[test]
fn test_ws_connection_send_text() {
    use runact_web::websocket::connection::WebSocketConnection;
    use std::io::Cursor;

    let mut conn = WebSocketConnection::with_writer(Cursor::new(Vec::new()), Vec::new());
    conn.send_frame(&Frame {
        fin: true,
        opcode: OpCode::Text,
        masked: false,
        mask_key: [0; 4],
        payload: b"server says hi".to_vec(),
    })
    .unwrap();
    let output = conn.into_writer();

    let decoded = Frame::parse(&output).unwrap();
    assert_eq!(decoded.opcode, OpCode::Text);
    assert_eq!(
        std::str::from_utf8(&decoded.payload).unwrap(),
        "server says hi"
    );
}

#[test]
fn test_ws_connection_ping_pong() {
    use runact_web::websocket::connection::WebSocketConnection;
    use std::io::Cursor;

    let ping_frame = Frame {
        fin: true,
        opcode: OpCode::Ping,
        masked: false,
        mask_key: [0; 4],
        payload: b"ping".to_vec(),
    };
    let mut buf = Vec::new();
    buf.extend_from_slice(&ping_frame.encode());

    let mut conn = WebSocketConnection::with_writer(Cursor::new(buf), Vec::new());
    let frame = conn.read_frame().unwrap();
    assert_eq!(frame.opcode, OpCode::Ping);
    assert_eq!(frame.payload, b"ping");

    conn.send_pong(&frame.payload).unwrap();
    let output = conn.into_writer();
    let pong = Frame::parse(&output).unwrap();
    assert_eq!(pong.opcode, OpCode::Pong);
    assert_eq!(pong.payload, b"ping");
}

#[test]
fn test_ws_connection_close() {
    use runact_web::websocket::connection::WebSocketConnection;
    use std::io::Cursor;

    let close_frame = Frame {
        fin: true,
        opcode: OpCode::Close,
        masked: false,
        mask_key: [0; 4],
        payload: 1000u16.to_be_bytes().to_vec(),
    };
    let mut buf = Vec::new();
    buf.extend_from_slice(&close_frame.encode());

    let mut conn = WebSocketConnection::with_writer(Cursor::new(buf), Vec::new());
    let frame = conn.read_frame().unwrap();
    assert_eq!(frame.opcode, OpCode::Close);

    conn.send_close(1000).unwrap();
    let output = conn.into_writer();
    let close_resp = Frame::parse(&output).unwrap();
    assert_eq!(close_resp.opcode, OpCode::Close);
    assert_eq!(
        u16::from_be_bytes([close_resp.payload[0], close_resp.payload[1]]),
        1000
    );
}

// ---------------------------------------------------------------------------
// Router integration: upgrade handler on a route
// ---------------------------------------------------------------------------

#[test]
fn test_router_websocket_upgrade_route() {
    let mut router = Router::new();
    router.route(Method::Get, "/ws", |_req: Request| -> Response {
        Response {
            status: StatusCode::SwitchingProtocols,
            reason: "Switching Protocols".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: {
                let mut h = Headers::new();
                h.insert("Upgrade", "websocket");
                h.insert("Connection", "Upgrade");
                h.insert("Sec-WebSocket-Accept", "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
                h
            },
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
