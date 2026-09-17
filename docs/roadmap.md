# Runact Roadmap

## Overview

This document outlines the development phases for Runact. Runact is the runtime; PaperOS is a separate editor project built on top of it.

---

## Current Status

| Component | Status | Tests |
|-----------|--------|-------|
| **Runact** (runtime) | v1.0.0 released | 19+ test files across basic, compute, observability, resource, scheduler, timer |

**Paper** (editor) — Separate project, tracked at `paper/docs/roadmap.md`

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
Editor Runtime (PaperOS)
    ↓
Programmable Environment
    ↓
Optional Distributed Runtime
```

The runtime must earn every layer of complexity.
