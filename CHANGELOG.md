# Changelog

All notable changes to Runact will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Documentation** — synced `AGENTS.md`, `docs/roadmap.md`, `docs/development-plan.md`, and `docs/architecture.md` with the v1.3.x codebase: corrected the version (v1.3.2), added `hotswap`, `net`, and `process` to the module layout, documented the `libc` and optional `libloading` dependencies, refreshed test counts (199 tests across 27 suites; 208 with `hot_code_loading`), marked the async runtime, HTTP, WebSocket, and hot code loading phases complete, and repaired the malformed `[1.3.1]`/`[1.3.2]` entries.

## [1.3.2] - 2026-09-27

### Fixed

- **SpawnListener vtable dispatch** — child actors spawned via `DynamicSupervisor`/`spawn_child` now properly invoke the vtable's `deserialize_fn`, `handle_message_fn`, and `drop_fn` via `unsafe` blocks. Previously the child loop polled the mailbox but did not actually dispatch to the actor handler. Added `StatePtr` wrapper (`unsafe impl Send + Sync`) to safely transport the raw state pointer across threads.

## [1.3.1] - 2026-09-27

### Added

- **Pattern 2 automatic spawning** — `DynamicSupervisor` now automatically spawns child actors per work item via `ctx.spawn_child()`. Added `SpawnListener` background thread that processes spawn requests using `CodeManager::get_current_vtable()`, ensuring new children always use the latest code version. Added `WorkPayload` trait for serializing work into child actor initial state.

## [1.3.0] - 2026-09-27

### Added

- **Hot code loading (Pattern 2)** — feature-gated module (`hot_code_loading` feature) providing Erlang/OTP-like hot code loading for `DynamicSupervisor`. `Runtime::load_actor_module()` loads a new version of an actor module from a dynamic library; after loading, **new child actors spawned by the supervisor automatically use the new code version** without affecting existing children. Existing children from before the upgrade continue with their original code until they complete. Includes `CodeManager` (version registry), `ActorFactory` (dynamic spawn with current-version vtables), `DynamicLibrary` (libloading wrapper), `HotSwappable` trait (serialize/deserialize state, version-aware), `DynamicSupervisor` (Pattern 2 — spawns a child actor per work item, always using the current version), `WorkerMessage`/`WorkItem`/`WorkerResponder` (supervisor message protocol), and `MigrationRegistry` (`register` + `migrate` for chaining state migrations between versions).

### Architecture

- Pattern 2 (dynamic supervision) with automatic spawning: `DynamicSupervisor` receives work items and spawns a child actor per item via `ctx.spawn_child()`. `ActorFactory::prepare_dynamic_spawn()` always retrieves the current version's vtable from `CodeManager`, so new child actors automatically run the latest code after an upgrade. Existing children keep their original vtable until they terminate.

## [1.2.2] - 2026-09-20

### Fixed

- **Scheduler idle CPU burn** — the work-stealing scheduler's worker loop spun on `thread::yield_now()` when all queues were empty, burning ~3+ cores at 100% CPU on an idle runtime. Workers now block on a `Condvar` (via `RunQueue::pop_timeout`) with a short timeout before retrying, consuming negligible CPU when idle.

## [0.2.0] - 2026-09-18

### Added

- **runact-web: WebSocket support** — RFC 6455 implementation in `runact-web::websocket`: frame parsing/encoding (per-frame masking, extended 16/64-bit lengths, all opcodes), handshake validation with `Sec-WebSocket-Accept` computation (SHA-1 + base64 using the standard RFC 6455 GUID), `WebSocketConnection<R, W>` over `Read`/`Write` with `send_pong`/`send_close` helpers, and `StatusCode::SwitchingProtocols` (101). Added `sha1` and `base64` dependencies.
- **runact-web: Async WebSocket** — `AsyncWebSocket` provides non-blocking frame I/O over any `Read + Write` stream. Uses dedicated reader/writer threads with a bounded channel and cooperative shutdown. Includes `Message` enum (Frame/Closed/Error), `AsyncWriter`, auto-pong for Ping.
- **runact-web: WebSocket server** — `WebSocketServer` handles the HTTP→WebSocket upgrade handshake, then runs a callback-based message loop. Supports echo, ping/pong, close frames over TCP.
- **runact-web: WebSocket heartbeats** — `WebSocketConfig` with `ping_interval`, `SendError` enum.
- **runact-web: WebSocket binary + control frames** — `send_binary`, `send_ping`, `send_pong`.
- **runact-web: WebSocket echo example** — `websocket_echo.rs` with 30s ping heartbeats.
- **runact-web: WebSocket chat example** — `websocket_chat.rs` broadcast chat server.
- **runact-web: WebSocket chat broadcast test** — `ws_chat.rs`.
- **runact-web: Runact TCP bridge** — `RunactTcpStream` adapter, `WebSocketServer` generic over stream type, `ws_runact_bridge.rs` test.
- **runact-web: WebSocketServer Connected event** — `ServerEvent::Connected` before reader loop.
- **runact-web: read_handshake_request helper** — retry-based read with 5s timeout.
- **runact-web: agent_server example + test** — `agent_server.rs`, `ws_agent_server.rs`.
- **runact-web: agent_api REST example + test** — `agent_api.rs`, `ws_agent_api.rs`.
- **runact-web: WebSocket message fragmentation** — RFC 6455 §5.4 reassembly, `ws_fragmentation.rs` tests.
- **runact-web: reader loop multi-frame processing** — processes all frames from a single `read()`.
- **Docs: reference manual** — "The Runact Runtime" book in `docs/book/` (20 chapters, PDF included).

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
