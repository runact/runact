# Changelog

All notable changes to Runact will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] - 2026-09-16

### Added

- **Cancellation** — `CancellationToken` with parent→child propagation for cooperative async task cancellation.
- **Task groups** — `TaskGroup` for structured concurrency: scoped task lifetimes, automatic cleanup on scope exit.
- **Sleep & timeout** — `Runtime::sleep()` and `Runtime::timeout()` as associated functions using crossbeam channels (no Tokio).
- **Actor ↔ async integration** — actors can `spawn_task` and receive results via `TaskHandle`; async tasks can send messages to actors.
- **Process runtime** — `ProcessSpawnOptions`, `ProcessHandle`, `ProcessOutput` for spawning OS processes with stdin/stdout/stderr, exit status, timeout, and cancellation.
- **TCP reactor** — `Reactor` using raw Linux `epoll` for OS readiness integration. `Interest`, `Readiness` types.
- **TCP API** — `TcpListener`, `TcpStream` with `bind`, `accept`, `connect`, `read`, `write`, `read_exact`, `read_to_end`, `shutdown`.
- **Stress tests** — 3 stress tests validating 100+ concurrent TCP connections.

### Changed

- `async-runtime.md` §9 updated: TCP networking IS in Runact core; HTTP/WebSocket/TLS/DNS are outside.
- `architecture.md` §29 updated: workspace approach (runact core + runact-web member).
- `development-plan.md` §48 references roadmap.md as authoritative phase source.
- `runtime-networking-plan.md` §27 references roadmap.md as authoritative phase source.

## [1.1.0] - 2026-09-16

### Added

- **Async tasks** — native executor for standard Rust `Futures` (no Tokio). `Runtime::spawn_task` returns a `TaskHandle` with `recv()`/`try_recv()`/`recv_timeout()`, a cached terminal outcome, and panic isolation (a panicking task surfaces `TaskError::Panic` without killing a worker). Shutdown sweeps all live tasks and resolves pending handles to `TaskError::ExecutorShutdown`.
- **`TaskId`** — unique identifier for a spawned async task.
- **`TaskError`** — task failure modes: `Panic(String)`, `ExecutorShutdown`, `Timeout`.

## [1.0.0] - 2026-09-15

### Added

- **Actor trait** — synchronous `fn handle(&mut self, msg, ctx)` with typed messages.
- **Runtime** — top-level coordinator: spawn, send, request, shutdown.
- **Work-stealing scheduler** — per-worker `RunQueue`, steal-half protocol, reduction-based cooperative yield.
- **Backpressure** — bounded mailbox (default 1000) with `MailboxFull` error on overflow.
- **Graceful shutdown** — `Runtime::shutdown()` drains remaining messages with configurable timeout.
- **Request-reply** — `Runtime::request()` returns `RequestHandle` with `recv()`/`try_recv()`/`recv_timeout()`.
- **Actor-to-actor messaging** — `ActorContext::send_to()` and `ActorContext::reply()`.
- **Supervision** — `Supervisor`, `RestartStrategy::OneForOne`, `ChildSpec`, `RestartPolicy`.
- **Exponential restart backoff** — configurable `base_backoff`, capped at 30s.
- **Compute scheduler** — bounded thread pool for CPU-intensive tasks with panic isolation.
- **Task cancellation** — `ComputeHandle::cancel()` checked before/after execution.
- **Timers** — one-shot (`schedule_timer`) and periodic (`schedule_interval`) with drift correction.
- **Resources** — `ResourceHandle` trait, `Capability<H>`, `ResourceRegistry` for type-safe resource access.
- **Observability** — structured logging via `tracing` (spawn, message, crash, restart events).
- **RuntimeStats** — `runtime.stats()` returns actor count and request count.
- **`RuntimeConfig`** — configurable `mailbox_capacity`, `shutdown_timeout`, `ComputeConfig`.
- **`#[must_use]`** on all public types.
- **Benchmarks** — criterion-based benchmarks for spawn, latency, throughput, memory, scalability.
- **Documentation** — rustdoc on every public type/method, five guide documents.

### Architecture

- Workspace approach: `runact` core crate + `runact-web` as workspace member.
- No Tokio dependency — own runtime semantics.
- `crossbeam-channel` for mailboxes and inter-thread communication.
- `tracing` for structured logging.
- `thiserror` for error types.
- `serde` with `derive` feature for message serialization.
- Raw Linux `epoll` for TCP reactor (no Mio).

## [0.1.0] - 2026-08-01

### Added

- Initial scaffold with actor trait, runtime, scheduler, supervision, compute, timers, and resources.
