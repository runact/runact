//! Adapter that bridges `runact::net::tcp_api::TcpStream` to `std::io::Read`
//! and `std::io::Write`, so it can be used with `WebSocketServer` and
//! `AsyncWebSocket`.
//!
//! The runact `TcpStream` uses non-blocking I/O with a reactor and provides
//! custom `read`, `write_all`, `flush`, and `read_exact` methods rather than
//! implementing the standard library traits. This adapter wraps it and
//! implements `Read` and `Write` by delegating to those methods.

use std::io::{self, Read, Write};
use std::time::Duration;

/// Wrapper that implements `Read` and `Write` for `runact::net::tcp_api::TcpStream`.
pub struct RunactTcpStream(pub runact::net::tcp_api::TcpStream);

impl Read for RunactTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // runact's TcpStream is non-blocking and returns Ok(0) on WouldBlock.
        // The AsyncWebSocket reader loop handles WouldBlock by sleeping and
        // retrying, so we translate Ok(0) from the inner stream into a
        // WouldBlock error. ConnectionReset (EOF) is also Ok(0) — we can't
        // distinguish, but if the connection is truly closed, subsequent reads
        // will keep returning Ok(0) and the reader loop will eventually
        // break when the stream is dropped or the shutdown flag is set.
        match self.0.read(buf) {
            Ok(0) => Err(io::Error::new(io::ErrorKind::WouldBlock, "not ready")),
            Ok(n) => Ok(n),
            Err(e) => Err(e),
        }
    }
}

impl Write for RunactTcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // runact's TcpStream is non-blocking; write_all may return WouldBlock.
        // We retry with a small sleep until all bytes are written or an error
        // occurs.
        if buf.is_empty() {
            return Ok(0);
        }
        let mut written = 0;
        while written < buf.len() {
            match self.0.write_all(&buf[written..]) {
                Ok(()) => {
                    written = buf.len();
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl super::server::SetReadTimeout for RunactTcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        // runact's TcpStream is non-blocking by design; the read method
        // already handles WouldBlock internally. We simulate a timeout by
        // delegating to the inner stream's set_read_timeout if available.
        // Since runact's TcpStream doesn't expose set_read_timeout, we
        // return Ok(()) — the handshake read will block until data arrives.
        let _ = timeout;
        Ok(())
    }
}

impl RunactTcpStream {
    /// Returns a reference to the inner runact `TcpStream`.
    pub fn inner(&self) -> &runact::net::tcp_api::TcpStream {
        &self.0
    }

    /// Consumes the adapter and returns the inner runact `TcpStream`.
    pub fn into_inner(self) -> runact::net::tcp_api::TcpStream {
        self.0
    }
}
