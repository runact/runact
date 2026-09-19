# Chapter 12: TCP Runtime

Runact provides native TCP support via `runact::net::tcp_api`. This chapter
covers the TCP listener, stream API, and how TCP integrates with the actor
runtime.

---

## 12.1 Design Philosophy

Runact's TCP layer follows the design principle from
`docs/runtime-networking-plan.md`:

> TCP is the lowest-level network primitive.

Runact provides blocking-style TCP I/O (with non-blocking retries) that
actors can use directly. Higher-level protocols (HTTP, WebSocket, TLS,
DNS) are implemented in `runact-web` or application code.

### Threading Model

Each `TcpStream` is non-blocking at the OS level, but the API provides
blocking-style methods with internal retry loops. For concurrent handling,
each connection typically gets its own thread (or actor + reader/writer
threads).

## 12.2 TcpListener

```rust
use runact::net::tcp_api::TcpListener;

let listener = TcpListener::bind("127.0.0.1:8080")?;
```

### accept()

```rust
loop {
    let client = listener.accept()?;
    // Handle client (spawn thread, spawn actor, etc.)
}
```

`accept` blocks until a connection arrives. Each call returns a `TcpStream`
that is already set to non-blocking mode and registered with the I/O
reactor.

### local_addr()

```rust
let addr = listener.local_addr()?;
println!("Listening on {}", addr);
```

## 12.3 TcpStream

```rust
use runact::net::tcp_api::TcpStream;

let mut stream = TcpStream::connect("127.0.0.1:8080")?;
```

### Read

```rust
let mut buf = [0u8; 4096];
let n = stream.read(&mut buf)?;  // retries on WouldBlock
```

`read` returns `Ok(0)` on connection close. It retries on `WouldBlock`
internally (with a 1ms sleep), providing a blocking-style interface over
a non-blocking socket.

### write_all + flush

```rust
stream.write_all(b"HTTP/1.1 200 OK\r\n\r\nHello")?;
stream.flush()?;
```

### read_exact

```rust
let mut header = [0u8; 8];
stream.read_exact(&mut header)?;  // blocks until 8 bytes read
```

### read_to_end

```rust
let mut buf = Vec::new();
stream.read_to_end(&mut buf)?;  // reads until connection closes
```

`read_to_end` includes a heuristic: if it has read some data and then
sees `WouldBlock` for 200+ consecutive attempts, it returns what it has
rather than blocking forever.

### shutdown

```rust
stream.shutdown(std::net::Shutdown::Both)?;
```

## 12.4 Simple Echo Server

```rust
use runact::net::tcp_api::TcpListener;
use std::io::{Read, Write};
use std::thread;

let listener = TcpListener::bind("127.0.0.1:8080")?;

loop {
    let mut client = listener.accept()?;

    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match client.read(&mut buf) {
                Ok(0) => break,  // connection closed
                Ok(n) => {
                    client.write_all(&buf[..n]).unwrap();
                    client.flush().unwrap();
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    break;
                }
            }
        }
    });
}
```

## 12.5 TCP with Actors

The natural pattern is to spawn a per-connection actor, with separate
reader/writer threads that feed the actor's mailbox:

```rust
use runact::{Actor, ActorContext, ActorError, Runtime};
use runact::net::tcp_api::TcpListener;
use std::io::Read;
use std::thread;

struct Connection {
    stream: TcpStream,
}

impl Actor for Connection {
    type Message = NetEvent;
    fn handle(&mut self, msg: NetEvent, _ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        match msg {
            NetEvent::Data(data) => {
                // Process data, send response via writer thread
            }
            NetEvent::Closed => {
                // Cleanup
            }
        }
        Ok(())
    }
}

let mut runtime = Runtime::new()?;
let listener = TcpListener::bind("127.0.0.1:8080")?;

loop {
    let client = listener.accept()?;
    let conn_id = runtime.spawn(Connection { stream: client })?;
    // Spawn reader/writer threads...
}
```

This pattern — actor per connection with dedicated reader/writer threads —
is how `runact-web`'s WebSocket server works (see Chapter 14).

## 12.6 I/O Reactor

Under the hood, `TcpListener` and `TcpStream` use an epoll-based
`Reactor` (on Linux) that registers socket file descriptors for readiness
events. This allows the runtime to efficiently multiplex many connections.
