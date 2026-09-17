# Runact

A Rust-native concurrent runtime and application platform inspired by BEAM.

## Vision

Runact is one ecosystem, with multiple layers and crates. It provides the foundation for web applications, AI agents, remote AI agent servers, PaperOS, network services, background workers, CLI applications, real-time applications, and automation systems.

> **Runact is the platform. `runact-web` is Runact's web layer.**

See [Development Plan](docs/development-plan.md) for the full architecture.

## Features

- **Actor trait** — synchronous `fn handle(&mut self, msg, ctx)` with typed messages
- **Work-stealing scheduler** — per-worker `RunQueue`, steal-half protocol, reduction-based cooperative yield
- **Request-reply** — `Runtime::request()` with `RequestHandle::recv()`/`try_recv()`/`recv_timeout()`
- **Supervision** — `RestartStrategy::OneForOne` with exponential backoff
- **Compute pool** — offload CPU-intensive work to a thread pool with panic isolation and cancellation
- **Timers** — one-shot and periodic with drift correction
- **Cancellation** — cooperative `CancellationToken` with parent→child propagation
- **Structured concurrency** — `TaskGroup` for scoped task lifetimes
- **Async tasks** — native executor for standard Rust `Future`s with `Runtime::spawn_task`, `TaskHandle`, panic isolation, and deterministic shutdown
- **Process management** — spawn processes, stdin/stdout/stderr, exit status, timeout, cancellation
- **TCP networking** — reactor, `TcpListener`, `TcpStream` with OS readiness integration (Linux epoll)
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

- [Development Plan](docs/development-plan.md) — Full 53-section architecture and platform vision
- [Architecture](docs/architecture.md) — Full architectural document with runtime model, invariants, and development order
- [Async Runtime](docs/async-runtime.md) — Architectural boundary: Runact schedules asynchronous work, I/O libraries define it (Future executor, task lifecycle, cancellation, timers, task groups)
- [Vision](docs/vision.md) — Design priorities and long-term evolution
- [Roadmap](docs/roadmap.md) — Development phases and current status
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
