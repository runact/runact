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
- ✅ `docs/security.md` — Capability-based security
- ✅ `docs/adr/0001-beam-style-scheduler.md` — ADR for scheduler choice
- ✅ `docs/adr/0002-dual-scheduler-architecture.md` — ADR for dual scheduler
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

## Phase 7 — Programmability

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
Editor Runtime (PaperOS)
    ↓
Programmable Environment
    ↓
Optional Distributed Runtime
```

The runtime must earn every layer of complexity.
