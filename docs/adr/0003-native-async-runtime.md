# ADR-0003: Native Async Runtime

## Status

Proposed

## Date

2026-09-17

## Context

Runact needs a native async task system to support actor ↔ async task integration, I/O-heavy operations (WebSocket, HTTP, LSP), timers, cancellation, and structured concurrency for PaperOS and other applications.

Runact already owns its execution model: a BEAM-style actor scheduler with work stealing, reduction counting, a dedicated compute pool, and cooperative cancellation. The actor model is deliberately synchronous and cooperative — actors must never block a worker while waiting for I/O.

Two forces shape this decision:

1. **Runact should execute standard Rust `Future`s.** Users must be able to write `runtime.spawn(async { ... })` without learning a proprietary async model. The standard `Future`/`Poll`/`Waker`/`Context` interfaces are the contract.
2. **Runact must not depend on Tokio to implement its own executor.** Tokio is a complete async ecosystem; Runact is deliberately small ("Concurrency + actors + task execution + lifecycle"). Depending on Tokio would pull a second execution model, a second scheduler, and its entire dependency tree into the core.

Additionally, the existing timer service is thread-based (`TimerService` with a dedicated thread), which does not scale to thousands of timers the way a scheduler-integrated timer driver would. The async runtime needs a timer mechanism (timer wheel or priority queue + driver) that integrates with the executor rather than creating one OS thread per timer.

## Decision

Implement a **native Runact executor** that:

- Polls standard Rust `Future`s on the Runact scheduler infrastructure.
- Uses a Runact-specific `Waker` with an atomic state machine to prevent duplicate scheduling.
- Integrates async task runnable queues alongside actor runnable queues under the same global scheduler.
- Provides a `CancellationToken`-based cooperative cancellation model.
- Provides `sleep()`/`timeout()` via a centralized timer driver (start with the simpler correct implementation: priority queue + timer driver; consider a timer wheel only if benchmarks justify it).
- Keeps the compute pool separate from async workers.
- Initially ships no OS-level I/O reactor. External libraries (HTTP, WebSocket, TLS, filesystem) are integrated by being executed on the Runact executor — Runact provides execution, libraries provide networking.
- Uses standard library primitives and minimal dependencies. No Tokio in the Runact core.

## Consequences

### Positive

- Runact retains a single coherent execution model and ownership story.
- Users write ordinary `async`/`await` Rust code with no Tokio coupling in the core API.
- Pending async tasks consume no worker execution time (the core scheduling invariant).
- Timers integrate with the scheduler instead of one-thread-per-timer.
- The public API stays small: `Runtime::spawn`, `Runtime::compute`, `Runtime::sleep`, `Runtime::timeout`, `Runtime::task_group`, `TaskHandle`, `CancellationToken`.

### Negative

- Reimplementing an executor is real work and risk: waker memory management, duplicate-schedule prevention, fairness between actors and async tasks, and shutdown ordering all need careful handling.
- No full I/O reactor initially — applications must bring their own I/O libraries.
- Without Tokio, we forgo its mature ecosystem and battle-tested timer/IO implementations; integrations that need Tokio-compatible libraries must adapt them to a foreign executor (or run them on their own runtime).

### Risks

- Executor correctness bugs (lost wakeups, double schedulings, waker leaks) are subtle and hard to find without extensive stress testing.
- Fairness between actors and async tasks may require benchmark-driven tuning.
- Forcing externally-started futures (e.g., library spawners that assume Tokio) onto a foreign executor can fail or panic; the adapter boundary (async-runtime.md §8, Phase 9) must be handled only when a concrete integration requires it.

## Alternatives Considered

### Use Tokio as the underlying runtime

Runact would wrap Tokio and expose actor semantics on top of it.

**Pros:**
- Mature, battle-tested executor, timers, and I/O stack.
- Huge ecosystem; easy integration of networking libraries.
- Less custom executor code to maintain.

**Cons:**
- Tokio types (spawners, wakers, timers, handles) tend to leak into public APIs.
- Two execution models (Tokio's scheduler + Runact's actor scheduler) competing for workers.
- Pulls a large dependency tree into a deliberately minimal core.
- Contradicts the existing "No Tokio" architecture and ADR-0001/0002 scheduler choices.

**Reason for rejection:** Runact already owns its scheduler and execution semantics. Tokio in the core would duplicate the scheduler, complicate fairness/ownership, and enlarge the dependency surface — without any concrete requirement that only Tokio can satisfy.

### Build a complete OS-level I/O reactor (epoll/kqueue/IOCP/io_uring)

**Pros:**
- Full control over I/O; no external library dependency for networking.

**Cons:**
- Extremely complex; platform-specific; months of work.
- Duplicates what mature libraries already provide.

**Reason for rejection:** Premature. Runact's job is execution and lifecycle, not protocol implementation (async-runtime.md §6, §9). Start with executor + standard Future/Waker + timer driver, then integrate real I/O libraries. If native networking is ever needed, isolate platform I/O behind an internal abstraction.

## References

- [Async Runtime Boundary](../async-runtime.md) — What Runact owns vs. what I/O libraries provide
- [Architecture](../architecture.md) — Runact architectural document, invariants, and development order
- [ADR-0001: Beam-Style Scheduler](0001-beam-style-scheduler.md)
- [ADR-0002: Dual-Scheduler Architecture](0002-dual-scheduler-architecture.md)
- [Vision](../vision.md) — Design priorities and long-term evolution