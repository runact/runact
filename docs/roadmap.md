# Runact Roadmap

## Overview

This document outlines the development phases for Runact. Each phase builds on the previous one. Runact is the runtime; Paper is a separate editor project built on top of it.

---

## Current Status

| Component | Status | Tests |
|-----------|--------|-------|
| **Runact** (runtime) | Phases 0–5 complete, v0.5–v0.9 complete, v1.0.0 released | 48/48 passing |
| **Paper** (editor) | Separate project — see `paper/docs/roadmap.md` | Compiles |

---

## Phase 0 — Architecture ✅

### Deliverables

- [x] `docs/architecture.md`
- [x] `docs/actors.md`
- [x] `docs/actor-communication.md`
- [x] `docs/roadmap.md`
- [x] Single-crate structure

---

## Phase 1 — Runtime Core ✅

### Deliverables

- [x] `src/actor/` — ActorId, Actor trait, ActorContext
- [x] `src/mailbox/` — Mailbox using crossbeam-channel
- [x] `src/scheduler/` — BEAM-style scheduler with work stealing
- [x] `src/runtime.rs` — Top-level coordinator
- [x] Message passing (fire-and-forget, request/reply)
- [x] Actor-to-actor communication
- [x] Graceful shutdown
- [x] Reduction counting (cooperative preemption)

---

## Phase 2 — Reliability ✅

### Deliverables

- [x] `src/supervision/` — Supervisor, ChildSpec, RestartStrategy
- [x] One-for-one restart strategy
- [x] Restart limits
- [x] Crash isolation

---

## Phase 3 — Compute ✅

### Deliverables

- [x] `src/compute/` — ComputeScheduler, ComputeHandle, ComputeError
- [x] Bounded queue
- [x] Task submission from actors
- [x] Panic isolation
- [x] Try_recv/recv_timeout for non-blocking polling

---

## Phase 4 — Timers ✅

### Deliverables

- [x] `src/timer/` — TimerService, TimerHandle, TimerId
- [x] One-shot timers
- [x] Periodic timers
- [x] Timer cancellation
- [x] Actor-context timer scheduling

---

## Phase 5 — Resources and Capabilities ✅

### Deliverables

- [x] `src/resource/` — ResourceHandle, Capability, ResourceRegistry
- [x] Generic capability wrapper
- [x] Type-safe resource registry

---

## Roadmap to v1

### v0.5 — API Hardening ✅

| Task | Status |
|------|--------|
| Audit public API surface — hide internals, finalize exports | ✅ |
| `#[must_use]` on critical return types | ✅ |
| `RuntimeConfig` re-exported | ✅ |
| Clippy clean (`-D warnings`) | ✅ |
| Zero dead code warnings | ✅ |

### v0.6 — Observability ✅

| Task | Status |
|------|--------|
| Structured logging via `tracing` (actor spawn, message send, crash) | ✅ |
| Actor metrics: message count, queue depth, reduction count | ✅ |
| `Runtime::stats()` — snapshot of runtime health | ✅ |
| Supervisor crash reporting with backtrace capture | ✅ |

### v0.7 — Robustness ✅

| Task | Status |
|------|--------|
| Backpressure: bounded mailbox with `MailboxFull` policy (drop/reject/block) | ✅ |
| Actor stop/shutdown: graceful drain before kill | ✅ |
| `Supervisor` restart backoff (exponential, configurable) | ✅ |
| Timer drift correction for periodic timers | ✅ |
| Compute pool: task timeout and cancellation | ✅ |

### v0.8 — Documentation ✅

| Task | Priority |
|------|----------|
| Rustdoc for every public type and method | ✅ |
| Guide: "Getting Started" — spawn actor, send message, receive reply | ✅ |
| Guide: "Supervision" — restart strategies, failure handling | ✅ |
| Guide: "Compute" — offloading CPU work | ✅ |
| Guide: "Timers" — scheduling messages | ✅ |
| Guide: "Resources" — capability-oriented access | ✅ |
| `cargo doc` builds clean (zero warnings) | ✅ |

### v0.9 — Benchmarks ✅

| Task | Priority |
|------|----------|
| 100K concurrent actors benchmark | ✅ (~1.5s spawn) |
| Message latency benchmark (< 10μs target) | ✅ (~6-13μs request-reply) |
| Work stealing throughput benchmark | ✅ |
| Memory per actor benchmark (< 1KB target) | ✅ (RSS tracking) |
| `cargo bench` with criterion | ✅ |

### v1.0 — Release ✅

| Task | Priority |
|------|----------|
| `CHANGELOG.md` | ✅ |
| CI: GitHub Actions (test, clippy, rustfmt, MSRV) | ✅ |
| MSRV policy: Rust 1.80 (last 3 stable releases) | ✅ |
| Security audit: zero `unsafe` in source | ✅ |
| `Cargo.toml` metadata (keywords, categories, readme) | ✅ |
| `README.md` | ✅ |

---

## Timeline

```
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

- 100K+ concurrent actors
- Message latency < 10μs
- Work stealing across cores
- No starvation
- Memory per actor < 1KB
- Supervisor restart reliability: 100%
- Crash isolation: verified

### Editor

- Working editor
- Responsive UI
- Real-world functionality
- Extensions work

> Editor milestones are tracked in `paper/docs/roadmap.md`.

---

## Long-Term Evolution

```
Runtime Core
    ↓
Reliability (Supervision)
    ↓
Compute Pool
    ↓
Timers and I/O
    ↓
Resources and Capabilities
    ↓
Editor Runtime (Paper)
    ↓
Programmable Environment
```

The runtime must earn every layer of complexity.
