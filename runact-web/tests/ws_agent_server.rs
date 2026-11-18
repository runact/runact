use runact_web::websocket::frame::OpCode;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Pick a free ephemeral address for the example server.
fn free_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    listener.local_addr().expect("local addr").to_string()
}

/// Path to a prebuilt example binary in the workspace target directory.
fn example_binary(name: &str) -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("workspace root")
                .join("target")
        });
    target.join("debug").join("examples").join(name)
}

/// Kills the example server when the test ends, including on panic.
struct ServerGuard(Child);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Connect once the server is accepting connections (max ~5s).
fn connect_until_ready(addr: &str) -> TcpStream {
    for _ in 0..50 {
        match TcpStream::connect(addr) {
            Ok(stream) => return stream,
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    panic!("server at {addr} did not become ready");
}

/// Integration test for the agent server example: verify a client can
/// connect, send a text message, and receive the echo response back.
#[test]
fn test_agent_server_echo() {
    // Run the prebuilt example directly. Invoking `cargo run` from inside
    // `cargo test` would block on the build lock held by the test harness.
    let addr = free_addr();
    let binary = example_binary("agent_server");
    assert!(
        binary.exists(),
        "missing {} — build examples first (`cargo build -p runact-web --examples`)",
        binary.display()
    );

    let child = Command::new(&binary)
        .env("AGENT_SERVER_ADDR", &addr)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn agent_server");
    let _guard = ServerGuard(child);

    let mut client = connect_until_ready(&addr);
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
}
