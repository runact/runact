# Changelog

All notable changes to Runact will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **runact-web: WebSocket support** — RFC 6455 implementation in `runact-web::websocket`: frame parsing/encoding (per-frame masking, extended 16/64-bit lengths, all opcodes), handshake validation with `Sec-WebSocket-Accept` computation (SHA-1 + base64 using the standard RFC 6455 GUID), `WebSocketConnection<R, W>` over `Read`/`Write` with `send_pong`/`send_close` helpers, and `StatusCode::SwitchingProtocols` (101). Added `sha1` and `base64` dependencies.
- **runact-web: Async WebSocket** — `AsyncWebSocket` provides non-blocking frame I/O over any `Read + Write` stream (e.g. `TcpStream`). Uses dedicated reader/writer threads with a bounded channel and cooperative shutdown, keeping I/O off scheduler workers. Includes `Message` enum (Frame/Closed/Error), `AsyncWriter` with non-blocking `send`/`send_close`, and auto-pong for incoming Ping frames.
- **runact-web: WebSocket server** — `WebSocketServer` handles the full HTTP→WebSocket upgrade handshake (101 Switching Protocols with RFC 6455 `Sec-WebSocket-Accept`), then runs a callback-based message loop. Provides `bind` (path-matching + upgrade validation), `accept` (performs handshake + returns `AsyncWebSocket`), and `accept_with_callback` (handshake + message loop with `ConnectionWriter` for replies). Supports echo, ping/pong, and close-frame roundtrips over real TCP.
- **runact-web: WebSocket heartbeats** — `WebSocketConfig` with `ping_interval` for periodic Ping frames to keep connections alive. `AsyncWebSocket::with_callback_and_config` and `WebSocketServer::accept_with_callback_and_config` accept the config. Includes `SendError` enum (Full/Closed) for proper error handling in non-blocking sends.
- **runact-web: WebSocket binary + control frames** — Added `send_binary`, `send_ping`, `send_pong` to `ConnectionWriter` and `AsyncWriter` for full RFC 6455 control frame support. Ping/Pong/Shutdown variants in the outbound channel.
- **runact-web: WebSocket echo example** — Runnable example binary `websocket_echo.rs` demonstrating a complete echo server with 30s ping heartbeats.
- **runact-web: WebSocket chat example** — Runnable example binary `websocket_chat.rs` demonstrating a multi-client broadcast chat server with peer registry and 30s ping heartbeats.
- **runact-web: WebSocket chat broadcast test** — Integration test `ws_chat.rs` verifying two-client broadcast: sender connects, sends text frame, receiver receives the broadcasted message.
- **runact-web: Runact TCP bridge** — `RunactTcpStream` adapter implements `Read`/`Write`/`SetReadTimeout` for `runact::net::tcp_api::TcpStream`, enabling `WebSocketServer` to accept connections from the runact async TCP runtime. Integration test `ws_runact_bridge.rs` verifies full broadcast over runact-managed TCP connections. `WebSocketServer` is now generic over `S: Read + Write + SetReadTimeout + Send + 'static`.
- **runact-web: WebSocketServer Connected event** — `ServerEvent::Connected` variant sent before the reader loop, allowing callbacks to register peer writers immediately upon handshake completion (fixes race condition in broadcast patterns).
- **runact-web: read_handshake_request helper** — `WebSocketServer::bind` now uses a retry-based read with 5s timeout, handling non-blocking streams (like `RunactTcpStream`) that return `WouldBlock` during handshake.
- **runact-web: agent_server example + test** — Phase 13 application: WebSocket server bridging runact's TCP listener to a supervised `AgentActor`. Demonstrates the full architecture from `docs/agents.md`: actor-managed LLM adapter with `MockAdapter`, `RuntimeSender` for cross-thread message delivery, and `ModelAdapter` trait for pluggable backends.
- **runact-web: agent_api REST example + test** — Phase 13 REST API server using `Router` with runact's `TcpListener`. CRUD endpoints for agent sessions (`GET/POST /api/agents`, `GET/DELETE /api/agents/:id`), shared `SessionStore` behind `Arc<Mutex>` (would be an Actor in production), HTTP request reader with Content-Length support.

## [1.2.1] - 2026-09-16

### Changed

- **Documentation** — comprehensive getting-started guide covering all v1.2.0 features (actors, async tasks, cancellation, process management, TCP networking).
- **Documentation** — reframed PaperOS-specific docs as Runact design patterns for web apps.
- **Documentation** — fixed architecture/async-runtime/development-plan inconsistencies.
- **Documentation** — updated supervision example to web-app-relevant scenario.

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
