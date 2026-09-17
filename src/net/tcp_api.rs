use crate::net::reactor::{Interest, Reactor};
use std::io::{Read, Write};
use std::net::{
    SocketAddr, TcpListener as StdTcpListener, TcpStream as StdTcpStream, ToSocketAddrs,
};
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct TcpListener {
    inner: StdTcpListener,
    reactor: Arc<Mutex<Reactor>>,
}

impl TcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> std::io::Result<Self> {
        let inner = StdTcpListener::bind(addr)?;
        inner.set_nonblocking(true)?;
        let reactor = Arc::new(Mutex::new(Reactor::new()?));
        let fd = inner.as_raw_fd();
        {
            let mut r = reactor.lock().unwrap();
            r.register(fd, Interest::READABLE)?;
        }
        Ok(Self { inner, reactor })
    }

    pub fn accept(&self) -> std::io::Result<TcpStream> {
        loop {
            match self.inner.accept() {
                Ok((stream, _addr)) => {
                    stream.set_nonblocking(true)?;
                    let fd = stream.as_raw_fd();
                    {
                        let mut r = self.reactor.lock().unwrap();
                        r.register(fd, Interest::READABLE | Interest::WRITABLE)?;
                    }
                    return Ok(TcpStream {
                        inner: stream,
                        reactor: Arc::clone(&self.reactor),
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        let fd = self.inner.as_raw_fd();
        let _ = self.reactor.lock().unwrap().unregister(fd);
    }
}

pub struct TcpStream {
    inner: StdTcpStream,
    reactor: Arc<Mutex<Reactor>>,
}

impl TcpStream {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> std::io::Result<Self> {
        let stream = StdTcpStream::connect(&addr)?;
        stream.set_nonblocking(true)?;
        let reactor = Arc::new(Mutex::new(Reactor::new()?));
        let fd = stream.as_raw_fd();
        {
            let mut r = reactor.lock().unwrap();
            r.register(fd, Interest::READABLE | Interest::WRITABLE)?;
        }
        Ok(Self {
            inner: stream,
            reactor,
        })
    }

    pub fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.inner.write_all(buf)
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }

    pub fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.inner.read(buf) {
            Ok(n) => Ok(n),
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::ConnectionReset =>
            {
                Ok(0)
            }
            Err(e) => Err(e),
        }
    }

    pub fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        let mut pos = 0;
        while pos < buf.len() {
            match self.inner.read(&mut buf[pos..]) {
                Ok(n) => pos += n,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let mut total = 0;
        let mut tmp = [0u8; 4096];
        let mut consecutive_would_blocks = 0;
        loop {
            match self.inner.read(&mut tmp) {
                Ok(0) => return Ok(total),
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    total += n;
                    consecutive_would_blocks = 0;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    consecutive_would_blocks += 1;
                    if total > 0 && consecutive_would_blocks > 200 {
                        return Ok(total);
                    }
                    if consecutive_would_blocks > 1000 {
                        return Ok(total);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::ConnectionReset => {
                    return Ok(total);
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
        self.inner.shutdown(how)
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let fd = self.inner.as_raw_fd();
        let _ = self.reactor.lock().unwrap().unregister(fd);
    }
}
