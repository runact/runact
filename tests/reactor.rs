use runact::net::{Interest, Reactor};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

#[test]
fn test_reactor_create() {
    let reactor = Reactor::new().expect("create reactor");
    assert_eq!(reactor.len(), 0);
}

#[test]
fn test_reactor_register_fd() {
    let mut reactor = Reactor::new().expect("create reactor");
    let (rx, _tx) = std::os::unix::net::UnixStream::pair().expect("pipe");
    let fd = rx.as_raw_fd();

    reactor.register(fd, Interest::READABLE).expect("register");
    assert_eq!(reactor.len(), 1);
}

#[test]
fn test_reactor_unregister_fd() {
    let mut reactor = Reactor::new().expect("create reactor");
    let (rx, _tx) = std::os::unix::net::UnixStream::pair().expect("pipe");
    let fd = rx.as_raw_fd();

    reactor.register(fd, Interest::READABLE).expect("register");
    reactor.unregister(fd).expect("unregister");
    assert_eq!(reactor.len(), 0);
}

#[test]
fn test_reactor_wait_readable() {
    let mut reactor = Reactor::new().expect("create reactor");
    let (rx, mut tx) = std::os::unix::net::UnixStream::pair().expect("pipe");
    let fd = rx.as_raw_fd();

    reactor.register(fd, Interest::READABLE).expect("register");

    tx.write_all(b"data").expect("send data");
    drop(tx);

    let events = reactor.wait(Duration::from_millis(100)).expect("wait");
    assert!(!events.is_empty());
    assert!(events[0].readiness.is_readable());
}

#[test]
fn test_reactor_wait_writable() {
    let mut reactor = Reactor::new().expect("create reactor");
    let (_rx, tx) = std::os::unix::net::UnixStream::pair().expect("pipe");
    let fd = tx.as_raw_fd();

    reactor.register(fd, Interest::WRITABLE).expect("register");

    let events = reactor.wait(Duration::from_millis(100)).expect("wait");
    assert!(!events.is_empty());
    assert!(events[0].readiness.is_writable());
}

#[test]
fn test_reactor_modifies_interest() {
    let mut reactor = Reactor::new().expect("create reactor");
    let (rx, mut tx) = std::os::unix::net::UnixStream::pair().expect("pipe");
    let fd = rx.as_raw_fd();

    reactor.register(fd, Interest::READABLE).expect("register");
    reactor
        .modify(fd, Interest::READABLE | Interest::WRITABLE)
        .expect("modify");

    tx.write_all(b"data").expect("send data");
    drop(tx);

    let events = reactor.wait(Duration::from_millis(100)).expect("wait");
    assert!(!events.is_empty());
    assert!(events[0].readiness.is_readable());
}

#[test]
fn test_reactor_wait_timeout() {
    let reactor = Reactor::new().expect("create reactor");
    let start = std::time::Instant::now();
    let events = reactor.wait(Duration::from_millis(50)).expect("wait");
    let elapsed = start.elapsed();

    assert!(events.is_empty());
    assert!(elapsed >= Duration::from_millis(50));
}

#[test]
fn test_reactor_token_association() {
    let mut reactor = Reactor::new().expect("create reactor");
    let (rx, mut tx) = std::os::unix::net::UnixStream::pair().expect("pipe");
    let fd = rx.as_raw_fd();

    reactor
        .register_with_token(fd, Interest::READABLE, 42)
        .expect("register");

    tx.write_all(b"data").expect("send data");
    drop(tx);

    let events = reactor.wait(Duration::from_millis(100)).expect("wait");
    assert_eq!(events[0].token, 42);
}

#[test]
fn test_reactor_multiple_fds() {
    let mut reactor = Reactor::new().expect("create reactor");
    let (rx1, mut tx1) = std::os::unix::net::UnixStream::pair().expect("pipe1");
    let (rx2, mut tx2) = std::os::unix::net::UnixStream::pair().expect("pipe2");

    reactor
        .register(rx1.as_raw_fd(), Interest::READABLE)
        .expect("reg1");
    reactor
        .register(rx2.as_raw_fd(), Interest::READABLE)
        .expect("reg2");

    tx1.write_all(b"a").expect("send1");
    tx2.write_all(b"b").expect("send2");
    drop(tx1);
    drop(tx2);

    let events = reactor.wait(Duration::from_millis(100)).expect("wait");
    assert_eq!(events.len(), 2);
}
