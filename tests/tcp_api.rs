use runact::net::tcp_api::{TcpListener, TcpStream};
use std::io::{Read, Write};

#[test]
fn test_tcp_listener_bind() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    assert!(listener.local_addr().is_ok());
}

#[test]
fn test_tcp_listener_accept() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    let _client = std::net::TcpStream::connect(addr).expect("connect");
    let _stream = listener.accept().expect("accept");
}

#[test]
fn test_tcp_stream_connect() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.write_all(b"hello").expect("write");
    stream.flush().expect("flush");
}

#[test]
fn test_tcp_stream_read_write() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    let mut stream = TcpStream::connect(addr).expect("connect");
    let (client_stream, _addr) = listener.accept().expect("accept");

    stream.write_all(b"ping").expect("write");
    stream.flush().expect("flush");

    let mut buf = [0u8; 4];
    let mut client = client_stream;
    client.read_exact(&mut buf).expect("read");
    assert_eq!(&buf, b"ping");

    client.write_all(b"pong").expect("write back");
    client.flush().expect("flush");

    let mut response = [0u8; 4];
    stream.read_exact(&mut response).expect("read response");
    assert_eq!(&response, b"pong");
}

#[test]
fn test_tcp_stream_read_to_end() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    let mut stream = TcpStream::connect(addr).expect("connect");
    let (mut client, _addr) = listener.accept().expect("accept");

    stream.write_all(b"hello world").expect("write");
    stream
        .shutdown(std::net::Shutdown::Write)
        .expect("shutdown write");

    let mut buf = Vec::new();
    client.read_to_end(&mut buf).expect("read to end");
    assert_eq!(buf, b"hello world");
}

#[test]
fn test_tcp_stream_partial_read() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    let mut stream = TcpStream::connect(addr).expect("connect");
    let (mut client, _addr) = listener.accept().expect("accept");

    stream.write_all(b"abcdef").expect("write");
    stream.flush().expect("flush");

    let mut buf = [0u8; 3];
    client.read_exact(&mut buf).expect("read 3 bytes");
    assert_eq!(&buf, b"abc");

    let mut buf2 = [0u8; 3];
    client.read_exact(&mut buf2).expect("read 3 more bytes");
    assert_eq!(&buf2, b"def");
}

#[test]
fn test_tcp_multiple_connections() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    let mut streams = Vec::new();
    for i in 0..5 {
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream
            .write_all(format!("msg{i}").as_bytes())
            .expect("write");
        stream.flush().expect("flush");
        streams.push(stream);
    }

    for _ in 0..5 {
        let mut accepted = listener.accept().expect("accept");
        let mut buf = [0u8; 4];
        accepted.read_exact(&mut buf).expect("read");
    }
}

#[test]
fn test_tcp_stream_shutdown() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    let mut stream = TcpStream::connect(addr).expect("connect");
    let (_client, _addr) = listener.accept().expect("accept");

    stream.write_all(b"data").expect("write");
    stream
        .shutdown(std::net::Shutdown::Write)
        .expect("shutdown");

    let mut buf = [0u8; 1];
    let result = stream.read(&mut buf);
    assert!(result.is_err() || result.unwrap() == 0);
}
