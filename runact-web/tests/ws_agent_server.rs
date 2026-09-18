use runact_web::websocket::frame::OpCode;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

/// Integration test for the agent server example: verify a client can
/// connect, send a text message, and receive the echo response back.
#[test]
fn test_agent_server_echo() {
    // Spawn the agent_server example as a subprocess
    let mut child = std::process::Command::new("cargo")
        .args(["run", "-p", "runact-web", "--example", "agent_server"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("spawn agent_server");

    // Wait for server to start
    thread::sleep(Duration::from_secs(2));

    let addr = "127.0.0.1:8080";
    let mut client = TcpStream::connect(addr).expect("connect to agent server");
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set timeout");

    // Perform WebSocket handshake (use a valid key)
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let request = format!(
        "GET /agent HTTP/1.1\r\n\
         Host: {addr}\r\n\
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
    let n = client.read(&mut resp_buf).expect("read response");
    let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
    assert!(resp_str.contains("101"), "expected 101 Switching Protocols");

    // Read welcome message
    let mut buf = [0u8; 256];
    let n = client.read(&mut buf).expect("read welcome");
    let frame = runact_web::websocket::frame::Frame::parse(&buf[..n]).expect("parse welcome");
    assert_eq!(frame.opcode, OpCode::Text);

    // Send a text frame (unmasked, server-side)
    let text = "hello agent";
    let frame = runact_web::websocket::frame::Frame {
        fin: true,
        opcode: OpCode::Text,
        masked: false,
        mask_key: [0; 4],
        payload: text.as_bytes().to_vec(),
    };
    client.write_all(&frame.encode()).expect("write frame");
    client.flush().expect("flush");

    // Read echo response
    let mut buf = [0u8; 256];
    let n = client.read(&mut buf).expect("read echo");
    let frame = runact_web::websocket::frame::Frame::parse(&buf[..n]).expect("parse echo");
    assert_eq!(frame.opcode, OpCode::Text);
    let response = String::from_utf8_lossy(&frame.payload);
    assert!(
        response.contains("hello agent"),
        "echo should contain message"
    );

    // Cleanup
    let _ = child.kill();
}
