use runact::net::tcp_api::TcpStream;
use std::io::{Read, Write};
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

const NUM_CONNECTIONS: usize = 500;

#[test]
fn test_stress_concurrent_connections() {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind");
    listener.set_nonblocking(false).expect("set blocking");
    let addr = listener.local_addr().unwrap();

    let server_count = Arc::new(AtomicUsize::new(0));
    let server_count_clone = server_count.clone();

    let server = thread::spawn(move || {
        for _ in 0..NUM_CONNECTIONS {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 8];
            stream.read_exact(&mut buf).expect("read");
            stream.write_all(&buf).expect("write back");
            stream.flush().expect("flush");
            server_count_clone.fetch_add(1, Ordering::SeqCst);
        }
    });

    let mut handles = Vec::with_capacity(NUM_CONNECTIONS);
    for i in 0..NUM_CONNECTIONS {
        let handle = thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect");
            let msg = format!("{i:08}");
            stream.write_all(msg.as_bytes()).expect("write");
            stream.flush().expect("flush");

            let mut buf = [0u8; 8];
            stream.read_exact(&mut buf).expect("read");
            assert_eq!(&buf, msg.as_bytes());
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("client thread panic");
    }
    server.join().expect("server thread panic");
    assert_eq!(server_count.load(Ordering::SeqCst), NUM_CONNECTIONS);
}

#[test]
fn test_stress_concurrent_read_write() {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind");
    listener.set_nonblocking(false).expect("set blocking");
    let addr = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        for _ in 0..100 {
            let (mut stream, _) = listener.accept().expect("accept");
            thread::spawn(move || {
                let mut buf = [0u8; 1024];
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            stream.write_all(&buf[..n]).expect("write");
                            stream.flush().expect("flush");
                        }
                        Err(_) => break,
                    }
                }
            });
        }
    });

    let mut handles = Vec::new();
    for i in 0..100 {
        let handle = thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect");
            let data = vec![i as u8; 4096];
            stream.write_all(&data).expect("write");
            stream
                .shutdown(std::net::Shutdown::Write)
                .expect("shutdown");

            let mut response = Vec::new();
            stream.read_to_end(&mut response).expect("read");
            assert_eq!(response.len(), 4096);
            assert!(response.iter().all(|&b| b == i as u8));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("client panic");
    }
    server.join().expect("server panic");
}

#[test]
fn test_stress_small_messages() {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind");
    listener.set_nonblocking(false).expect("set blocking");
    let addr = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        for _ in 0..50 {
            let (mut stream, _) = listener.accept().expect("accept");
            thread::spawn(move || {
                for _ in 0..100 {
                    let mut buf = [0u8; 4];
                    match stream.read_exact(&mut buf) {
                        Ok(()) => {
                            stream.write_all(&buf).expect("write");
                            stream.flush().expect("flush");
                        }
                        Err(_) => break,
                    }
                }
            });
        }
    });

    let mut handles = Vec::new();
    for _ in 0..50 {
        let handle = thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect");
            for j in 0..100u32 {
                let msg = j.to_le_bytes();
                stream.write_all(&msg).expect("write");
                stream.flush().expect("flush");

                let mut buf = [0u8; 4];
                stream.read_exact(&mut buf).expect("read");
                assert_eq!(buf, msg);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("client panic");
    }
    server.join().expect("server panic");
}
