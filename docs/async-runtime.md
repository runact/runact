# Runact Async Runtime Boundary

## Goal

Define a strict architectural boundary for Runact.

Runact is a lightweight actor/runtime system inspired by BEAM, implemented in Rust. It should provide concurrency, scheduling, task lifecycle, supervision, cancellation, and resource management without becoming a complete application-level async ecosystem.

The core principle is:

> **Runact schedules asynchronous work; I/O libraries define asynchronous work.**

---

# 1. What Runact Owns

Runact owns the execution and lifecycle of concurrent work.

### Actor Runtime

* Actor creation
* Actor lifecycle
* Actor mailbox
* Typed messages
* Bounded mailboxes
* Backpressure
* Actor scheduling
* Cooperative yielding
* Actor supervision
* Panic isolation
* Actor restart/stop policies

### Async Task Runtime

Runact should provide a small native task abstraction based on standard Rust futures.

Own:

* Task creation
* Task IDs
* Task lifecycle
* Future polling
* Waker integration
* Task scheduling
* Task cancellation
* Task handles
* Task groups
* Task supervision
* Runtime shutdown
* Fairness between tasks
* Bounded task concurrency

Use standard Rust primitives:

```rust
Future
Poll
Waker
Context
Pin
```

Do NOT invent a new async programming model.

---

# 2. Compute Runtime

CPU-heavy work must be separated from ordinary async tasks.

Runact owns:

```text
ComputePool
```

Examples:

* parsing
* indexing
* syntax analysis
* compression
* hashing
* large calculations
* CPU-heavy AI preprocessing
* expensive transformations

API concept:

```rust
runtime.compute(|| {
    expensive_calculation()
});
```

CPU-heavy work must never monopolize actor workers or async workers.

---

# 3. Timers

Runact owns runtime timers.

Provide primitives such as:

```rust
runtime.sleep(duration)
runtime.timeout(duration, future)
```

Start with a simple correct timer implementation.

A timer wheel can be introduced later if profiling demonstrates the need.

---

# 4. Cancellation

Cancellation is a Runact responsibility.

Provide:

```rust
CancellationToken
```

Conceptually:

```rust
let token = runtime.cancellation_token();

token.cancel();
token.is_cancelled();
```

Cancellation must be cooperative.

Never forcibly kill arbitrary Rust threads or tasks.

---

# 5. Structured Concurrency

Runact should provide task groups.

Example:

```text
Workspace
   └── TaskGroup
       ├── indexing task
       ├── search task
       ├── LSP task
       └── AI task
```

When the workspace closes:

```text
Workspace closing
      ↓
Cancel TaskGroup
      ↓
Wait for tasks
      ↓
Release resources
```

This prevents orphaned background tasks.

---

# 6. What Runact Does NOT Own

Runact must NOT become an application-level networking framework.

Do NOT put these inside Runact core:

```text
HTTP
HTTPS
WebSocket
TLS
DNS
HTTP client
HTTP server
FTP
SSH
SMTP
Database protocols
LSP protocol
Git protocol
```

These belong to specialized libraries.

For example:

```text
reqwest       → HTTP
tokio-tungstenite → WebSocket
rustls        → TLS
trust-dns     → DNS
sqlx          → database
tower         → service middleware
```

Runact should execute their asynchronous operations where possible, rather than reimplementing them.

---

# 7. Critical Async Boundary

The architectural boundary should look like this:

```text
                 RUNACT
┌──────────────────────────────────────────┐
│                                          │
│  Actors                                  │
│  Scheduler                               │
│  Async Tasks                             │
│  Future Polling                          │
│  Wakers                                  │
│  Cancellation                            │
│  Task Groups                             │
│  Timers                                  │
│  Supervision                             │
│  Compute Pool                            │
│  Backpressure                            │
│  Shutdown                                │
│                                          │
└───────────────────┬──────────────────────┘
                    │
                    │ Future / Waker / TaskHandle
                    │
                    ▼
          ASYNC I/O BOUNDARY
                    │
                    ▼
┌──────────────────────────────────────────┐
│          External Libraries              │
│                                          │
│ HTTP                                     │
│ WebSocket                                │
│ TLS                                      │
│ DNS                                      │
│ Filesystem                               │
│ Database                                 │
│ LSP                                      │
│ Git                                      │
└──────────────────────────────────────────┘
```

Runact provides execution.

Libraries provide protocols and I/O implementations.

---

# 8. Tokio Boundary

Do NOT make Tokio a fundamental dependency of Runact core initially.

However, Runact must acknowledge an important fact:

> Polling a Tokio-based future from Runact does not automatically provide the Tokio runtime/reactor that some libraries require.

Therefore, external async ecosystems need adapters.

Architecture:

```text
PaperOS
   │
   ▼
Runact
   │
   ├── Native Runact tasks
   │
   └── Runtime Adapter
          │
          ▼
       Tokio
          │
          ├── HTTP
          ├── WebSocket
          └── other Tokio-based I/O
```

The adapter isolates Tokio from the Runact core.

---

# 9. What Belongs Where

Runact core provides:

```text
TCP networking (TcpListener, TcpStream, Reactor)
Process management (spawn, stdin/stdout/stderr)
```

These are fundamental runtime primitives. TCP is the lowest-level network primitive; process management is required for AI agents and system integration.

Avoid application-level protocols in Runact core:

```text
HTTP
WebSocket
TLS
DNS
```

These belong to specialized libraries or higher-level crates (e.g., `runact-web`).

Instead expose generic execution APIs:

```rust
runtime.spawn(future)
runtime.compute(job)
runtime.sleep(duration)
runtime.timeout(duration, future)
```

Example:

```rust
let task = runtime.spawn(async move {
    let response = http_client.get(url).send().await?;
    Ok::<_, Error>(response)
});
```

The HTTP client belongs to the HTTP library.

Runact only manages execution and lifecycle.

---

# 10. Actor + Async Integration

Actors should NOT fundamentally become async actors.

Use this model:

```text
                 Actor
                   │
          receives message
                   │
                   ▼
             Start I/O Task
                   │
                   ▼
          Actor continues work
                   │
                   │
             async I/O
                   │
                   ▼
          Task completes
                   │
                   ▼
          Send result message
                   │
                   ▼
                Actor
```

Example:

```rust
Actor receives:

FetchDocument
       ↓
spawn async task
       ↓
actor remains schedulable
       ↓
HTTP request waiting
       ↓
HTTP completes
       ↓
DocumentFetched(result)
       ↓
actor processes result
```

An actor must not block an actor worker while waiting for I/O.

---

# 11. Async Task Lifecycle

Use an explicit lifecycle:

```text
Created
   ↓
Scheduled
   ↓
Running
   ↓
Waiting
   │
   └──────────────┐
                  │ wake
                  ▼
               Runnable
                  │
                  ▼
               Running
                  │
                  ▼
              Completed
```

Failure:

```text
Running → Failed
```

Cancellation:

```text
Running
   ↓
Cancelling
   ↓
Cancelled
```

A pending task must consume essentially no worker execution time.

---

# 12. Waker Integration

Runact must implement a Runact-specific Waker.

When a future returns:

```rust
Poll::Pending
```

the task is not continuously polled.

The Waker marks the task runnable and schedules it again.

Conceptually:

```text
Future
  │
  └── Poll::Pending
          ↓
        Waker
          ↓
   mark task runnable
          ↓
      scheduler
          ↓
       worker
          ↓
     poll again
```

Prevent duplicate scheduling using an atomic task state or equivalent mechanism.

---

# 13. Worker Separation

Runact should logically separate:

```text
Actor Workers
Async Workers
Compute Workers
```

Conceptually:

```text
             Scheduler
          /      |       \
         /       |        \
    Actors     Async     Compute
     pool       pool       pool
```

The exact implementation may evolve.

The important invariant is:

> CPU-heavy work must not starve actors or asynchronous I/O tasks.

---

# 14. Backpressure

Runact must not encourage unlimited task creation.

Bad:

```rust
for item in millions {
    runtime.spawn(process(item));
}
```

without any concurrency bound.

Prefer:

```text
Input
  ↓
Bounded queue
  ↓
Limited concurrent tasks
  ↓
Results
```

Backpressure should exist at:

* actor mailboxes
* task queues
* compute queues
* external work submission

---

# 15. Shutdown

Runtime shutdown must be structured.

Recommended order:

```text
Stop accepting new work
        ↓
Cancel task groups
        ↓
Stop actors
        ↓
Wait for async tasks
        ↓
Stop async workers
        ↓
Stop compute workers
        ↓
Release runtime resources
```

Support shutdown deadlines so one broken task cannot keep the entire runtime alive forever.

---

# 16. Error and Panic Isolation

Actor panic:

```text
Actor panic
    ↓
Supervisor
    ↓
Restart / Stop / Escalate
```

Async task failure should remain attached to the task handle/group.

Compute job failure must not crash unrelated runtime workers.

The runtime itself should remain alive when an individual actor/task/job fails.

---

# 17. Public API Direction

Keep the public API small.

Target concepts:

```rust
Runtime
Actor
ActorRef
Task
TaskHandle
TaskGroup
CancellationToken
ComputePool
```

Runtime methods may include:

```rust
Runtime::new()
Runtime::spawn()
Runtime::compute()
Runtime::sleep()
Runtime::timeout()
Runtime::task_group()
Runtime::shutdown()
```

Avoid exposing internal scheduler structures.

---

# 18. PaperOS Relationship

PaperOS uses Runact as its execution foundation.

Architecture:

```text
                 PaperOS
┌──────────────────────────────────────────┐
│                                          │
│ Editor                                   │
│ Terminal                                 │
│ Filesystem                               │
│ Git                                      │
│ LSP                                      │
│ Search                                   │
│ AI                                       │
│ Database                                 │
│ Extensions                               │
│                                          │
└────────────────────┬─────────────────────┘
                     │
                     ▼
                  Runact
┌──────────────────────────────────────────┐
│ Actors                                   │
│ Tasks                                    │
│ Scheduler                                │
│ Cancellation                             │
│ Timers                                   │
│ Supervision                              │
│ Compute                                  │
└────────────────────┬─────────────────────┘
                     │
                     ▼
             External Libraries
```

PaperOS owns application-level services.

Runact owns concurrency and lifecycle.

---

# 19. Core Design Rule

When deciding whether something belongs in Runact, ask:

> "Is this about executing and managing concurrent work, or is it about implementing a particular application protocol?"

If it is about:

```text
scheduling
lifecycle
cancellation
supervision
tasks
actors
timers
fairness
backpressure
compute
```

it probably belongs in Runact.

If it is about:

```text
HTTP
TLS
WebSocket
DNS
Git
SQL
LSP
SSH
```

it belongs outside Runact.

---

# 20. Implementation Strategy

Do NOT attempt to build a complete async ecosystem.

Implement in this order:

### Phase 1 — Future Task

Implement:

```text
Task
TaskHandle
Executor
Future polling
```

### Phase 2 — Waker

Implement:

```text
Runact Waker
Runnable queue
Wake deduplication
```

### Phase 3 — Scheduler Integration

Integrate async tasks with the Runact scheduler.

### Phase 4 — Cancellation

Implement:

```text
CancellationToken
Task cancellation
```

### Phase 5 — Timers

Implement:

```text
sleep()
timeout()
```

### Phase 6 — Actor Integration

Allow actors to spawn async tasks and receive results as messages.

### Phase 7 — Task Groups

Implement structured concurrency.

### Phase 8 — Compute Pool

Separate CPU-heavy work from normal async execution.

### Phase 9 — Runtime Adapters

Create optional adapters for external runtimes such as Tokio when required by external libraries.

---

# 21. Tests

At minimum test:

```text
Future completion
Future Pending → Wake → Resume
Multiple tasks
Task cancellation
Task timeout
Timer accuracy
Task groups
Actor + async integration
Actor panic isolation
Compute isolation
Runtime shutdown
Backpressure
10,000+ tasks
100,000+ wakeups
Mixed actor/task workloads
```

Stress test the scheduler under mixed workloads.

---

# 22. Architectural Invariants

The canonical invariant set is in [Architecture](architecture.md) §35. The async-runtime-relevant invariants are:

1. Runact core does not implement HTTP.
2. Runact core does not implement WebSocket.
3. Runact core does not implement TLS.
4. Runact core does not implement DNS.
5. Runact core does not depend fundamentally on Tokio.
6. Runact executes standard Rust futures.
7. Pending futures consume no worker execution time.
8. Actors do not block while waiting for asynchronous I/O.
9. CPU-heavy work runs outside normal async/actor workers.
10. Cancellation is cooperative.
11. Task lifecycle is observable.
12. Runtime shutdown is structured.
13. External I/O libraries remain replaceable.

See [Architecture §35](architecture.md#35-architectural-invariants) for the full set (27 invariants).

---

# Final Principle

Runact should be:

> **A concurrency and lifecycle runtime, not an application framework.**

The clean boundary is:

```text
Actors
Tasks
Schedulers
Wakers
Timers
Cancellation
Supervision
Compute
Backpressure
Lifecycle
        ↓
   RUNACT BOUNDARY
        ↓
Future / Waker / Adapter
        ↓
HTTP / WebSocket / TLS / DNS / DB / LSP / Git
```

Keep Runact small.

Do not recreate Tokio.

Do not recreate BEAM.

Build the smallest Rust-native runtime that gives PaperOS and future projects a strong actor/task/concurrency foundation.

## See Also

- [Architecture](architecture.md) — Full architectural document with runtime model, invariants, and development order
- [ADR-0003: Native Async Runtime](adr/0003-native-async-runtime.md) — Rationale for a native Future executor without a Tokio core dependency
- [Runtime](runtime.md) — Top-level Runtime API, RequestHandle, RuntimeConfig, RuntimeStats
- [Scheduler](scheduler.md) — BEAM-style scheduler with work stealing
- [Vision](vision.md) — Design priorities and long-term evolution