# Runact

A Rust-native actor runtime combining BEAM-style lightweight processes, messaging, scheduling, and supervision with Rust's ownership model.

## Features

- **Actor trait** — synchronous `fn handle(&mut self, msg, ctx)` with typed messages
- **Work-stealing scheduler** — per-worker run queues, cooperative yield via reduction counting
- **Request-reply** — `Runtime::request()` with `RequestHandle::recv()`/`try_recv()`/`recv_timeout()`
- **Supervision** — `RestartStrategy::OneForOne` with exponential backoff
- **Compute pool** — offload CPU-intensive work to a thread pool with panic isolation and cancellation
- **Timers** — one-shot and periodic with drift correction
- **Resources** — type-safe capability-based resource management
- **Backpressure** — bounded mailbox with `MailboxFull` error on overflow
- **Observability** — structured logging via `tracing`

## Quick Start

```rust
use runact::{Actor, ActorId, ActorContext, ActorError, Runtime};

struct Greeter;

impl Actor for Greeter {
    type Message = String;

    fn handle(&mut self, msg: String, _ctx: &mut ActorContext) -> Result<(), ActorError> {
        println!("Hello, {}!", msg);
        Ok(())
    }
}

let mut runtime = Runtime::new().unwrap();
let id = runtime.spawn(Greeter).unwrap();
runtime.send(id, "world".to_string()).unwrap();
```

## Documentation

- [Getting Started](docs/guides/getting-started.md)
- [Supervision](docs/guides/supervision.md)
- [Compute](docs/guides/compute.md)
- [Timers](docs/guides/timers.md)
- [Resources](docs/guides/resources.md)
- [Actor Communication Principles](docs/actor-communication.md)
- [API Reference](https://docs.rs/runact)

## MSRV

Rust 1.80 (last 3 stable releases policy).

## License

MIT OR Apache-2.0
