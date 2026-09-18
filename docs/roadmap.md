# Runact Roadmap

## Overview

This document outlines the development phases for Runact. Runact is the runtime; PaperOS is a separate editor project built on top of it.

---

## Current Status

| Component | Status | Tests |
|-----------|--------|-------|
| **Runact** (runtime) | v1.2.1 | 108 tests across actor, async, cancellation, compute, observability, process, reactor, resource, scheduler, stress, tcp_api, timer |
| **runact-web** (HTTP) | v0.1.0 | 34 tests for HTTP Request, Response, Headers; 32 WebSocket tests (frame, async, server, chat, runact bridge) |
| **Phase 9** (Runtime + Networking) | ✅ Complete | cancellation, timers, actor-async integration, process runtime, TCP reactor, TCP API, stress tests |
| **Phase 10** (HTTP) | ✅ Complete | HTTP/1.1 parsing, request/response types, headers |
| **Phase 12** (WebSocket) | ✅ Complete | Upgrade, frames, connection lifecycle, async I/O, heartbeats, runact TCP bridge, examples |

**Paper** (editor) — Separate project, tracked at `paper/docs/roadmap.md`

Full development plan: [Development Plan](development-plan.md)

---

## Phase 0 — Architecture ✅

### Deliverables

- ✅ `docs/architecture.md` — Full architectural document (v1.0.0)
- ✅ `docs/actors.md` — Actor trait, ActorId, ActorContext
- ✅ `docs/actor-communication.md` — 24 communication principles
- ✅ `docs/scheduler.md` — BEAM-style scheduler with work stealing
- ✅ `docs/supervision.md` — Supervision trees, restart strategies
- ✅ `docs/runtime.md` — Runtime API, RequestHandle, RuntimeConfig
- ✅ `docs/vision.md` — Vision and design priorities
- ✅ `docs/roadmap.md` — This document
- ✅ `docs/async-runtime.md` — Async runtime boundary: what Runact owns vs. what I/O libraries provide
- ✅ `docs/security.md` — Capability-based security
- ✅ `docs/adr/0001-beam-style-scheduler.md` — ADR for scheduler choice
- ✅ `docs/adr/0002-dual-scheduler-architecture.md` — ADR for dual scheduler
- ✅ `docs/adr/0003-native-async-runtime.md` — ADR for native Future executor (no Tokio core dependency)
- ✅ Single-crate structure

---

## Phase 1 — Runtime Core ✅

### Deliverables

- ✅ `src/actor/` — ActorId, Actor trait, ActorContext
- ✅ `src/mailbox/` — Mailbox with backpressure policies
- ✅ `src/scheduler/` — BEAM-style scheduler with work stealing
- ✅ `src/runtime.rs` — Top-level coordinator
- ✅ Message passing (fire-and-forget, request/reply)
- ✅ Actor-to-actor communication
- ✅ Graceful shutdown
- ✅ Reduction counting (cooperative preemption)

---

## Phase 2 — Reliability ✅

### Deliverables

- ✅ `src/supervision/` — Supervisor, ChildSpec, RestartStrategy
- ✅ One-for-one restart strategy with exponential backoff
- ✅ Restart limits (`max_restarts`, `within` window)
- ✅ Crash isolation for compute tasks

---

## Phase 3 — Compute ✅

### Deliverables

- ✅ `src/compute/` — ComputeScheduler, ComputeHandle, ComputeError
- ✅ Bounded queue (soft limit via `queue_capacity`)
- ✅ Task submission from actors via `ctx.spawn_compute`
- ✅ Panic isolation at worker boundary
- ✅ `try_recv`/`recv_timeout` for non-blocking polling
- ✅ Cooperative cancellation via `ComputeHandle::cancel()`

---

## Phase 4 — Timers ✅

### Deliverables

- ✅ `src/timer/` — TimerService, TimerHandle, TimerId
- ✅ One-shot timers (`schedule_timer`)
- ✅ Periodic timers (`schedule_interval`)
- ✅ Timer cancellation (`cancel_timer`)
- ✅ Actor-context timer scheduling (`ctx.schedule_timer`)
- ✅ Drift correction for periodic timers

---

## Phase 5 — Resources and Capabilities ✅

### Deliverables

- ✅ `src/resource/` — ResourceHandle, Capability, ResourceRegistry
- ✅ Generic capability wrapper with owner tracking
- ✅ Type-safe resource registry (`TypeId`-based)
- ✅ Thread-safe resources (`Send + Sync`)

---

## Phase 6 — Editor Runtime (PaperOS)

PaperOS is a separate project. Runact provides the foundation.

- ✅ Runact is runtime-independent of any editor
- ✅ APIs are stable and documented

---

## Phase 7 — Async Runtime

Planned. Runact will extend the runtime with a native async task system executing standard Rust `Future`s on its own executor, within the strict boundary defined by [Async Runtime](async-runtime.md): Runact schedules asynchronous work; I/O libraries define asynchronous work. See also [ADR-0003](adr/0003-native-async-runtime.md).

Implemented in the order defined by async-runtime.md §20:

### Deliverables

- Phase 1 — `Task`, `TaskHandle`, Executor, future polling
- Phase 2 — Runact waker, runnable queue, wake deduplication (atomic task state)
- Phase 3 — Async task scheduler integration
- Phase 4 — Cancellation (`CancellationToken`, task cancellation)
- Phase 5 — Timers (`sleep()`, `timeout()`)
- Phase 6 — Actor ↔ async integration (`ctx.spawn(async { ... })`, result delivered as a message)
- Phase 7 — Task groups (structured concurrency)
- Phase 8 — Compute pool separation (CPU-heavy work off the async/actor workers)
- Phase 9 — Optional runtime adapters (e.g. Tokio) only when a concrete library requires one

Success criteria:

- Pending async task consumes no worker execution time (async-runtime.md §22, invariant 7)
- Cancel 10,000 tasks without leaks; rapid spawn/cancel cycles stable
- 10,000-task and 100,000-wakeup stress tests pass (test list in async-runtime.md §21)
- Shutdown deterministically terminates all owned tasks within `shutdown_timeout`
- Fair mixed actor/async/compute workloads (invariant 9: CPU-heavy work never starves actors or async I/O)

---

## Phase 8 — Programmability

Future work:

- Extension API
- Scripting integration
- Distributed actors (local semantics must be stable first)

---

## Phase 10 — HTTP ✅

Build:

```text
runact-web
```

Implemented:

- ✅ HTTP/1.1 `Request` parsing and encoding
- ✅ HTTP/1.1 `Response` parsing and encoding
- ✅ Case-insensitive `Headers` type
- ✅ `Method` enum (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)
- ✅ `StatusCode` enum with known codes + `Custom(u16)`
- ✅ 34 acceptance tests (headers, request, response)

See [Development Plan](development-plan.md) §48 Phase 9.

---

## Phase 11 — Web Framework

Add:

```text
Router
Handlers
Middleware
Extractors
Streaming
Static files
```

See [Development Plan](development-plan.md) §48 Phase 10.

---

## Phase 12 — WebSocket

Add:

```text
Upgrade
Frames
Connection lifecycle
Streaming
Backpressure
Cancellation
```

### Status: ✅ Complete

Implemented:
- ✅ RFC 6455 frame parsing/encoding (all opcodes, masking, extended lengths)
- ✅ WebSocket handshake (`Sec-WebSocket-Accept` with SHA-1 + base64)
- ✅ `WebSocketServer` — generic over `S: Read + Write + SetReadTimeout + Send + 'static`
- ✅ `AsyncWebSocket` — non-blocking frame I/O with reader/writer/ping threads
- ✅ `ConnectionWriter` with `send_text`, `send_binary`, `send_close`, `send_ping`, `send_pong`
- ✅ `WebSocketConfig` with `ping_interval` for heartbeat support
- ✅ `RunactTcpStream` adapter bridging `runact::net::tcp_api::TcpStream` to `Read`/`Write`
- ✅ `ServerEvent` enum (`Connected`, `Frame`, `Closed`, `Error`)
- ✅ Example binaries: `websocket_echo.rs`, `websocket_chat.rs`
- ✅ Tests: `ws_server.rs` (4), `async_websocket.rs` (5), `websocket.rs` (21), `ws_chat.rs` (1), `ws_runact_bridge.rs` (1)

See [Development Plan](development-plan.md) §48 Phase 11.

---

## Phase 13 — Real Applications

Build:

```text
REST API
WebSocket server
AI agent server
Remote PaperOS server
```

### Status: In Progress

**AI agent server** — `examples/agent_server.rs` demonstrates the full agent lifecycle over WebSocket:
- `AgentActor` (runact `Actor`) wraps a pluggable `ModelAdapter`
- `RuntimeSender` delivers user messages from WebSocket callbacks to the actor
- Built on the `RunactTcpStream` bridge from Phase 12
- Integration test `ws_agent_server.rs` verifies end-to-end WebSocket echo through the actor

See [Development Plan](development-plan.md) §48 Phase 12.

---

## Phase 9 — Runtime + Networking Extension ✅

See [Runtime + Networking Plan](runtime-networking-plan.md) for the full design document.

This phase extends Runact into a small, Rust-native concurrency runtime for PaperOS desktop applications, remote AI-agent servers, and async network services.

### Core Principle

> Runact manages execution, concurrency, lifecycle, and low-level I/O readiness. Higher-level libraries implement protocols and applications.

### Deliverables

**Phase 9a — Cancellation & Task Groups** ✅
- `CancellationToken` — cooperative cancellation
- `TaskGroup` — structured concurrency
- Parent → child cancellation propagation

**Phase 9b — Timers** ✅
- `runtime.sleep(duration)` — associated function
- `runtime.timeout(duration, future)` — associated function
- Crossbeam channel-based implementation

**Phase 9c — Actor ↔ Async Integration** ✅
- `actor → spawn_task → result message`
- Actor remains responsive while async task waits

**Phase 9d — Process Runtime** ✅
- `runact-process` — spawn processes, stdin/stdout/stderr
- Exit status, timeout, cancellation, graceful termination
- Prevent orphaned processes on agent cancellation

**Phase 9e — TCP Reactor** ✅
- `runact-net` — native async TCP with OS readiness integration
- Linux `epoll` first, abstraction for future platforms
- `TcpListener`, `TcpStream`, read/write/connect/accept

**Phase 9f — Stress Testing** ✅
- 500 concurrent TCP connections
- Mixed workloads (actors + tasks + TCP + compute)
- Slow/fast clients, partial writes, cancellation

### Architectural Invariants

1. Runact is NOT an HTTP/WebSocket/TLS framework
2. Runact is NOT an AI framework
3. Runact executes standard Rust futures
4. Actors and async tasks remain distinct concepts
5. CPU-heavy work uses the compute pool
6. Async I/O never blocks an actor worker
7. Cancellation is cooperative
8. Structured concurrency prevents orphaned tasks
9. TCP is the lowest-level network primitive
10. HTTP/WebSocket remain outside Runact
11. Tokio is optional, not fundamental
12. Runact must remain useful independently of PaperOS

### Success Criteria

- Parent cancellation reaches all children
- Pending timers consume no worker time
- Actor remains responsive while async task waits
- Cancelled agent cannot leave orphaned processes
- TcpStream can asynchronously wait for readiness
- Multiple concurrent TCP connections work without blocking workers
- No orphaned tasks, no orphaned processes

### Crate Architecture (future)

```text
runact/
├── runact-core/      # actors, mailbox, scheduler, supervision
├── runact-runtime/   # executor, tasks, wakers, cancellation, timers
├── runact-compute/   # CPU-heavy work pool
├── runact-net/       # TCP, reactor, readiness, sockets
└── runact-process/   # process lifecycle, stdin/stdout/stderr
```

---

## Timeline

```text
Now ─────── v0.5 ──── v0.6 ──── v0.7 ──── v0.8 ──── v0.9 ──── v1.0
            │         │         │         │         │         │
            │         │         │         │         │         └─ Runact v1.0
            │         │         │         │         └─ Benchmarks
            │         │         │         └─ Documentation
            │         │         └─ Robustness (backpressure, restart)
            │         └─ Observability (tracing, metrics)
            └─ API Hardening (typed messages, error types)
```

---

## Success Metrics

### Runtime

- 100K+ concurrent actors (benchmarked in `benches/runact_bench.rs`)
- Message latency < 10μs (request-reply benchmarked)
- Work stealing across cores (scheduler tests)
- No starvation (reduction counting)
- Memory per actor < 1KB (RSS tracking benchmarked)
- Supervisor restart reliability: 100% (supervisor tests)
- Crash isolation: verified (compute tests)

---

## Long-Term Evolution

```text
Runtime Core
    ↓
Reliability (Supervision)
    ↓
Compute Pool
    ↓
Timers and Cancellation
    ↓
Resources and Capabilities
    ↓
Async Runtime (Future Executor, Task Groups)
    ↓
Process Runtime
    ↓
TCP Networking (Reactor, TcpListener, TcpStream)
    ↓
HTTP (runact-web)
    ↓
Web Framework (Router, Middleware, Extractors)
    ↓
WebSocket
    ↓
Real Applications (REST API, AI Agent Server, PaperOS)
```

The runtime must earn every layer of complexity.

See [Development Plan](development-plan.md) for the full 53-section architecture document.
