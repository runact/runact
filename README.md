# Runact

A Rust-native actor runtime combining BEAM-style lightweight processes, messaging, scheduling, and supervision with Rust's ownership model.

## Features

- **Actor trait** — synchronous `fn handle(&mut self, msg, ctx)` with typed messages
- **Work-stealing scheduler** — per-worker `RunQueue`, steal-half protocol, reduction-based cooperative yield
- **Request-reply** — `Runtime::request()` with `RequestHandle::recv()`/`try_recv()`/`recv_timeout()`
- **Supervision** — `RestartStrategy::OneForOne` with exponential backoff
- **Compute pool** — offload CPU-intensive work to a thread pool with panic isolation and cancellation
- **Timers** — one-shot and periodic with drift correction
- **Resources** — type-safe capability-based resource management
- **Backpressure** — bounded mailbox with `MailboxFull` error on overflow
- **Async tasks** — native executor for standard Rust `Futures` with `Runtime::spawn_task`, `TaskHandle` (`recv`/`try_recv`/`recv_timeout`), panic isolation, and deterministic shutdown; cancellation, task timers, and task groups are planned ([boundary](docs/async-runtime.md))
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

- [Architecture](docs/architecture.md) — Full architectural document with runtime model, invariants, and development order
- [Async Runtime](docs/async-runtime.md) — Architectural boundary: Runact schedules asynchronous work, I/O libraries define it (Future executor, task lifecycle, cancellation, timers, task groups)
- [Vision](docs/vision.md) — Design priorities and long-term evolution
- [Getting Started](docs/guides/getting-started.md) — Walkthrough of core APIs
- [Supervision](docs/guides/supervision.md) — Supervision trees and restart strategies
- [Compute](docs/guides/compute.md) — CPU-intensive work offloading
- [Timers](docs/guides/timers.md) — One-shot and periodic timers
- [Resources](docs/guides/resources.md) — Capability-based resource management
- [Actor Communication Principles](docs/actor-communication.md) — 24 principles for message flow and invariants
- [Scheduler](docs/scheduler.md) — BEAM-style scheduler with work stealing
- [Runtime API](docs/runtime.md) — Top-level Runtime API reference
- [Actors](docs/actors.md) — Actor trait, ActorId, ActorContext, ownership model
- [Security](docs/security.md) — Capability-based security model
- [Object Model](docs/object-model.md) — Core runtime types reference
- [Protocol](docs/protocol.md) — Message envelope and request/reply protocol
- [API Reference](https://docs.rs/runact)

## MSRV

Rust 1.85 (edition 2024).

## License

MIT OR Apache-2.0
