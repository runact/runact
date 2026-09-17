# Changelog

All notable changes to Runact will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

- Single crate (no workspace).
- No Tokio dependency — own runtime semantics.
- `crossbeam-channel` for mailboxes and inter-thread communication.
- `tracing` for structured logging.
- `thiserror` for error types.
- `serde` with `derive` feature for message serialization.

## [0.1.0] - 2026-08-01

### Added

- Initial scaffold with actor trait, runtime, scheduler, supervision, compute, timers, and resources.
