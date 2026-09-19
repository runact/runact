# Chapter 20: Architectural Decisions

This chapter summarizes the key architectural decisions documented in the
ADR series (`docs/adr/`).

---

## 20.1 ADR-0001: BEAM-Style Scheduler

**Decision**: Use a BEAM-style scheduler with work-stealing.

**Context**: Runact needs to run thousands of lightweight actors
concurrently across N worker threads.

**Decision**: Each worker maintains a run queue. When a worker is idle,
it steals work from another worker's queue. Actors are scheduled using
reduction counting (a budget of work per turn).

**Consequences**:
- Fair scheduling across all actors.
- Low overhead for idle actors (parked, no polling).
- Natural scaling with CPU cores.

## 20.2 ADR-0002: Dual Scheduler Architecture

**Decision**: Separate actor scheduler from async task executor, but share
worker threads.

**Context**: Actors and async tasks have different scheduling requirements.
Actors need reduction-based fairness; async tasks need Waker-driven
rescheduling.

**Decision**: Both run on the same pool of worker threads, but each has
its own run queue and scheduling policy. Actors use reduction counting;
async tasks use the standard `Poll`/`Waker` mechanism.

**Consequences**:
- Shared worker pool avoids thread overhead.
- Isolation prevents one type from starving the other.
- Simplification: no separate thread pools to manage.

## 20.3 ADR-0003: Native Async Runtime (No Tokio)

**Decision**: Implement a native Future executor. Do not depend on Tokio.

**Context**: Tokio is the dominant Rust async runtime, but it is heavy
and opinionated. Runact's core should be self-contained.

**Decision**: Implement `Future` polling, `Waker`, `Context`, `Poll`
directly. Provide `spawn_task`, `sleep`, and `timeout` APIs that mirror
Tokio's surface but use Runact's own scheduler.

**Consequences**:
- No Tokio dependency in core.
- Can interoperate with Tokio at the application layer if needed.
- Full control over scheduling semantics (reduction budgeting).

## 20.4 Additional Decisions

### Compute Pool Separation

**Decision**: CPU-bound work goes to a separate compute pool, never on
scheduler workers.

**Rationale**: A CPU-bound `handle` call would block the worker,
starving all other actors on that thread.

### Capability-Based Security

**Decision**: Resources are accessed via unforgeable capabilities
(`Capability<T>`, `ResourceHandle<T>`), not string names.

**Rationale**: The actor model's isolation model aligns with
capability-based security. Only actors holding a capability can access
the resource.

### Cooperative Cancellation

**Decision**: Cancellation is cooperative — tasks check
`CancellationToken::is_cancelled()` and exit gracefully.

**Rationale**: Force-killing threads is unsafe in Rust (no destructors
run). Cooperative cancellation ensures clean state.
