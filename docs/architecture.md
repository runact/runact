# Runact — Architectural Document

**Project:** Runact
**Language:** Rust
**Type:** Lightweight actor/concurrency runtime
**Primary Goal:** BEAM-inspired process management with Rust ownership, safety, explicit resource lifetime, and efficient CPU-bound computation.

---

## 1. Vision

Runact is a lightweight actor runtime for Rust that provides a programming model based on:

* lightweight actors/processes
* isolated actor state
* asynchronous message passing
* bounded mailboxes
* supervision
* work-stealing scheduling
* cooperative execution
* request/reply communication
* timers
* cancellation
* resource/capability management
* dedicated CPU-compute execution
* observability

The goal is **not** to reproduce the BEAM internally.

The goal is to provide the useful process-oriented programming model of BEAM while preserving Rust's:

* ownership
* borrowing
* type safety
* deterministic destruction
* zero-cost abstractions where practical
* explicit resource management

---

## 2. Core Design Principle

The fundamental abstraction is:

> An actor owns its state and communicates with other actors through messages.

An actor should not normally share mutable state with another actor.

Conceptually:

```text
Actor A
 ├── State A
 ├── Mailbox A
 └── Behavior A

Actor B
 ├── State B
 ├── Mailbox B
 └── Behavior B

A ───── Message ─────> B
B ───── Message ─────> A
```

Ownership of data may move between actors through messages.

Shared state is an explicit design decision rather than the default.

---

## 3. Runtime Model

Runact consists of several major layers.

```text
                    ┌──────────────────────┐
                    │     Application      │
                    └──────────┬───────────┘
                               │
                    ┌──────────▼───────────┐
                    │       Actors         │
                    └──────────┬───────────┘
                               │
               ┌───────────────┼────────────────┐
               │               │                │
        ┌──────▼─────┐ ┌──────▼──────┐ ┌──────▼──────┐
        │ Mailboxes  │ │  Scheduler  │ │ Supervision │
        └────────────┘ └──────┬──────┘ └─────────────┘
                               │
                     ┌─────────▼─────────┐
                     │ Worker Threads    │
                     └─────────┬─────────┘
                               │
              ┌────────────────┴────────────────┐
              │                                 │
       ┌──────▼──────┐                  ┌──────▼──────┐
       │ Actor Work  │                  │ Compute Pool│
       └─────────────┘                  └─────────────┘
```

---

## 4. Actor

An actor consists conceptually of:

```text
Actor
 ├── Identity
 ├── State
 ├── Mailbox
 ├── Behavior
 ├── Lifecycle
 ├── Cancellation state
 └── Runtime metadata
```

An actor must not require an operating-system thread.

Thousands or millions of actors should theoretically be multiplexed over a much smaller number of worker threads.

The actor trait is defined as:

```rust
pub trait Actor: Send + 'static {
    type Message: Send + 'static;

    fn handle(
        &mut self,
        message: Self::Message,
        context: &mut ActorContext,
    ) -> Result<(), ActorError>;
}
```

The handler is synchronous and cooperative — it must return quickly, process one message, and yield back to the scheduler.

---

## 5. Actor State

Actor state is actor-owned.

Example:

```rust
struct Counter {
    value: u64,
}
```

The actor processes messages:

```text
Increment
Get
Reset
```

Only the actor modifies:

```text
Counter.value
```

Other actors communicate with it through messages.

This provides a natural ownership boundary.

---

## 6. Actor Identity

Every actor receives a stable runtime identity.

```rust
pub struct ActorId(u64);
```

The identity allows:

* sending messages (`Runtime::send`)
* request/reply (`Runtime::request`)
* supervision (`Supervisor`)
* tracing (via `tracing` spans)
* cancellation (via `Cancellation`)
* lifecycle tracking (`ActorInfo`)
* debugging (`Runtime::inspect_actor`)

Actor IDs do not expose internal memory addresses as their public identity.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ActorId(u64);
```

---

## 7. Mailbox

Each actor owns a mailbox.

The mailbox is the boundary between producers and the actor.

```text
Producer
   │
   ▼
┌───────────────┐
│   Mailbox     │
│ [msg][msg][ ] │
└───────┬───────┘
        │
        ▼
     Actor
```

Runact supports bounded mailboxes. The default capacity is 1000 messages.

```text
capacity = 1000
```

The purpose of bounded mailboxes is to prevent uncontrolled memory growth.

---

## 8. Backpressure

When a mailbox is full, the sender must have an explicit policy.

Possible policies:

```text
Reject
Block
Wait asynchronously
Drop
Timeout
Return error
```

The default API prefers explicit failure rather than silently losing messages.

Example:

```rust
runtime.send(actor_id, message)?;
```

Possible error:

```rust
RuntimeError::MailboxFull(ActorId)
```

Backpressure is part of the runtime semantics. The `send` method uses `try_send` internally and returns `MailboxFull` when the bounded mailbox is at capacity. External threads may use `send_blocking` to wait until space is available.

---

## 9. Message Passing

Messages are typed.

Example:

```rust
enum Message {
    Increment,
    Reset,
    Get,
}
```

Messages may contain owned data.

Example:

```rust
struct ProcessFile {
    path: PathBuf,
    contents: Vec<u8>,
}
```

Ownership can move into the actor.

This avoids requiring shared mutable memory.

---

## 10. Rust Ownership Model

Runact embraces Rust ownership instead of trying to hide it.

Preferred model:

```text
Producer owns data
       │
       │ move
       ▼
Message owns data
       │
       │ move
       ▼
Actor owns data
```

Instead of:

```text
Arc<Mutex<T>>
```

for every piece of state.

Shared state remains possible when genuinely required.

Runact does not artificially prevent:

```rust
Arc<T>
Arc<Mutex<T>>
RwLock<T>
Atomic<T>
```

but they are not the fundamental programming model.

---

## 11. Scheduler

The scheduler is responsible for executing runnable actors.

```text
Worker 1
 ├── Actor A
 ├── Actor D
 └── Actor F

Worker 2
 ├── Actor B
 └── Actor E

Worker 3
 ├── Actor C
 └── Actor G
```

Actors are not permanently assigned to a worker.

The scheduler uses a BEAM-style design with one run queue per worker thread.

```rust
pub struct Scheduler {
    queues: Vec<RunQueue>,
    next_worker: AtomicUsize,
    handles: Vec<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}
```

---

## 12. Work Stealing

Runact uses work stealing to balance load across workers.

```text
Worker A
Queue:
[A1][A2][A3][A4]

Worker B
Queue:
[B1]

Worker B becomes idle

        │ steal
        ▼

Worker B
Queue:
[A4][A3]
```

When a worker's local queue is empty, it steals half of the tasks from a victim queue. This allows workers to balance uneven workloads.

```rust
pub(crate) fn steal_half(&self) -> Vec<Box<dyn FnOnce() + Send>> {
    let mut queue = self.queue.lock().unwrap();
    let len = queue.len();
    if len <= 1 {
        return Vec::new();
    }
    let steal_count = len / 2;
    queue.drain(..steal_count).collect()
}
```

---

## 13. Cooperative Scheduling

Actor execution is cooperative.

An actor receives a limited amount of execution budget measured in reductions.

```text
Actor A
   │
   ├── message 1
   ├── message 2
   ├── message 3
   ├── message 4
   │
   └── yield
```

This prevents one actor from monopolizing a worker.

Runact uses a reduction/budget mechanism inspired by BEAM. The default is 4000 reductions before yielding:

```rust
pub const MAX_REDUCTIONS: u64 = 4000;
```

The exact reduction accounting is an implementation detail.

---

## 14. Important Scheduling Rule

Runact distinguishes between two kinds of work:

### Short actor work

Examples:

* update state
* process command
* update cache
* route message
* emit event

### Long CPU work

Examples:

* compilation
* compression
* image processing
* cryptography
* indexing
* AI inference
* large mathematical computation

Long CPU work must not block an actor worker. It is offloaded to the compute pool.

---

## 15. Compute Pool

Runact provides a dedicated compute pool for CPU-intensive work.

```text
Actor
  │
  │ submit job
  ▼
Compute Pool
 ├── CPU Worker 1
 ├── CPU Worker 2
 ├── CPU Worker 3
 └── CPU Worker 4
```

The actor can submit a `ComputeJob` and receive the result later:

```rust
ctx.spawn_compute(move || expensive_calculation())?;
```

Example flow:

```text
Actor
  │
  ├── submit computation
  │
  └── continue processing messages

Compute Worker
  │
  └── performs expensive calculation

Result
  │
  ▼

Actor
```

This is critical for long-running workloads.

---

## 16. Actor Must Not Block on Compute

Avoid:

```text
Actor
  │
  ├── start expensive calculation
  │
  ├── WAIT
  │
  └── cannot process messages
```

Prefer:

```text
Actor
  │
  ├── submit calculation
  │
  ├── continue processing messages
  │
  ├── process other messages
  │
  └── receive calculation result
```

This preserves actor responsiveness.

The `ComputeHandle` provides polling methods:

```rust
pub fn try_recv(&self) -> Option<Result<T, ComputeError>>;
pub fn recv(self) -> Result<T, ComputeError>;
pub fn recv_timeout(self, timeout: Duration) -> Result<T, ComputeError>;
pub fn cancel(&self);
```

---

## 17. Request / Reply

Runact supports request/reply.

```text
Actor A
   │
   │ Request
   ▼
Actor B
   │
   │ Reply
   ▼
Actor A
```

The reply mechanism does not require blocking an OS thread.

Implementation:

```text
Request
 ├── request id
 ├── payload
 └── reply channel
```

A bounded result channel (capacity 1) is used. The actor calls `ctx.reply()` to send the reply back.

```rust
pub struct RequestHandle {
    id: u64,
    receiver: crossbeam_channel::Receiver<Box<dyn std::any::Any + Send>>,
}
```

---

## 18. Deadlock Prevention

Synchronous request chains can create deadlocks.

```text
A waits for B
B waits for C
C waits for A
```

Therefore:

* asynchronous messaging is the primary model (`send`)
* synchronous `request` is implemented through asynchronous messaging with a reply channel — the actor never blocks a worker
* timeout support exists (`recv_timeout`)
* cancellation exists (`ComputeHandle::cancel`)

The fundamental invariant:

> **An actor must never synchronously wait for another actor while occupying a Runact scheduler worker.**

---

## 19. Cancellation

Operations support cancellation where practical.

```text
Task
 │
 ├── Running
 │
 ├── CancelRequested
 │
 ├── Completed
 │
 └── Cancelled
```

Cancellation distinguishes between:

```text
cooperative cancellation
```

and:

```text
forced termination
```

Rust code executing arbitrary CPU instructions cannot safely be forcibly interrupted at an arbitrary instruction boundary.

Therefore long-running compute jobs cooperate with cancellation via a flag that is checked before and after execution:

```rust
pub fn cancel(&self) {
    self.cancelled.store(true, Ordering::Relaxed);
}
```

---

## 20. Timeouts

Request/reply and asynchronous jobs support timeouts.

```rust
let reply = handle.recv_timeout(Duration::from_secs(5))?;
```

Timeouts result in explicit runtime errors (`RecvTimeoutError::Timeout`).

Timeouts are particularly important for:

* remote operations
* extension calls
* compute jobs
* shutdown
* supervision
* resource acquisition

---

## 21. Panic Handling

A panic inside a compute task does not bring down the runtime.

```text
Actor
  │
  │ panic (in compute task)
  ▼
Runtime catches failure
  │
  ├── record failure
  ├── notify (via ComputeResult::Panic)
  ├── clean up actor resources
  └── actor continues
```

Compute workers catch panics at the worker boundary:

```rust
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (task.job)()));
```

Panics are surfaced as `ComputeError::WorkerPanic(msg)`.

Supervisor strategies:

```text
Restart
Stop
Escalate
```

The supervisor decides the policy.

---

## 22. Supervision

Actors can form supervision relationships.

```text
Supervisor
 ├── FileIndexer
 ├── SearchEngine
 ├── GitService
 └── LanguageServer
```

If `FileIndexer` crashes, the supervisor may restart only `FileIndexer`.

---

## 23. Supervision Tree

Runact supports hierarchical supervision with `RestartStrategy::OneForOne`.

```text
Application
│
├── UI Supervisor
│   ├── Window Manager
│   └── UI Bridge
│
├── Editor Supervisor
│   ├── Buffer
│   ├── Search
│   └── LSP
│
└── System Supervisor
    ├── File Service
    ├── Git Service
    └── Terminal
```

Supervisor configuration:

```rust
pub enum RestartStrategy {
    OneForOne {
        max_restarts: usize,
        within: Duration,
        base_backoff: Duration,
    },
}
```

---

## 24. Lifecycle

Actors have explicit lifecycle states.

```text
Created
   ↓
Starting
   ↓
Running
   ↓
Stopping
   ↓
Stopped
```

Failure:

```text
Running
   │
   └── Failed
          │
          ├── Restart
          └── Stop
```

The runtime tracks actor lifecycle via `ActorInfo` and `tracing` events.

---

## 25. Timers

Runact provides runtime timers.

```text
send message after 1 second
repeat every 5 seconds
timeout request
schedule cleanup
```

Timers integrate with the actor system. One-shot and periodic timers are supported with drift correction (reschedule based on previous scheduled time, not current time).

Timers can be scheduled from actors:

```rust
ctx.schedule_timer(Duration::from_secs(1), WakeUpMessage)?;
ctx.schedule_interval(Duration::from_secs(5), Tick)?;
```

Or from the runtime:

```rust
runtime.schedule_timer(Duration::from_secs(1), actor_id, message)?;
runtime.schedule_interval(Duration::from_secs(5), actor_id, message)?;
```

Timers can be cancelled by ID:

```rust
runtime.cancel_timer(timer_id);
ctx.cancel_timer(timer_id);
```

---

## 26. Resources

Actors can own resources.

```text
Actor owns resource
       │
       ▼
Actor stops
       │
       ▼
Resource cleanup
```

This fits Rust's deterministic `Drop` semantics.

Resources are managed through `Capability<H>` which wraps a `ResourceHandle` with its owning `ActorId`.

---

## 27. Capabilities

Capabilities represent permissions/resources.

```text
FileSystemCapability
NetworkCapability
ProcessCapability
DatabaseCapability
UiCapability
```

An actor receives only the capabilities it needs.

Example:

```text
FileIndexer
 ├── ReadFile capability
 └── DirectoryWatch capability

AI Extension
 ├── ReadFile
 ├── Process
 └── Network
```

This becomes important for untrusted extensions.

```rust
pub struct Capability<H: ResourceHandle> {
    owner: ActorId,
    handle: Arc<H>,
}
```

---

## 28. Observability

Runact exposes:

* actor lifecycle events (via `tracing`)
* mailbox metrics (`ActorInfo.mailbox_depth`)
* scheduler metrics (worker count, task counts)
* worker utilization
* compute pool utilization
* message latency (via benchmarks)
* actor execution time (via `tracing` spans)
* crashes (via `tracing::error!`)
* restarts (via supervisor backoff)
* queue saturation (via `MailboxFull` errors)
* tracing spans (`tracing::info_span!`)

Tracing is integrated without making tracing mandatory for every application.

```rust
let stats = runtime.stats();
// RuntimeStats { actor_count, request_count }
```

---

## 29. Runtime Layers

Conceptual modules:

```text
runact/
├── src/
│   ├── actor/
│   │   ├── mod.rs
│   │   ├── id.rs          # ActorId
│   │   ├── trait_def.rs   # Actor trait
│   │   ├── context.rs     # ActorContext
│   │   └── error.rs       # ActorError
│   ├── mailbox/
│   │   ├── mod.rs
│   │   └── queue.rs       # BackpressurePolicy, Mailbox, MailboxConfig
│   ├── scheduler/
│   │   ├── mod.rs
│   │   ├── reduction.rs   # ReductionCounter
│   │   ├── run_queue.rs   # RunQueue
│   │   ├── work_stealing.rs  # Scheduler
│   │   └── worker.rs      # Worker
│   ├── supervision/
│   │   ├── mod.rs
│   │   ├── child.rs       # ChildSpec, RestartPolicy
│   │   ├── strategy.rs    # RestartStrategy
│   │   ├── supervisor.rs  # Supervisor
│   │   └── supervisor_actor.rs  # SupervisorActor
│   ├── compute/
│   │   ├── mod.rs
│   │   ├── handle.rs      # ComputeHandle
│   │   ├── scheduler.rs   # ComputeScheduler, ComputeConfig
│   │   └── task.rs        # Task, TaskId, ComputeError, ComputeResult
│   ├── timer/
│   │   ├── mod.rs
│   │   ├── service.rs     # TimerService, TimerHandle, TimerId
│   │   └── types.rs       # Timer, TimerConfig
│   ├── resource/
│   │   ├── mod.rs
│   │   ├── capability.rs  # Capability
│   │   ├── handle.rs      # ResourceHandle
│   │   └── registry.rs    # ResourceRegistry
│   └── runtime.rs            # Runtime, RuntimeConfig, RuntimeStats, RequestHandle
```

These are conceptual boundaries. The project is a single crate.

---

## 30. API Philosophy

Prefer small APIs.

The user should be able to understand the basic model quickly.

```rust
let mut runtime = Runtime::new().unwrap();

let actor = runtime.spawn(MyActor::new()).unwrap();

runtime.send(actor, Message::Hello).unwrap();

runtime.shutdown().unwrap();
```

Advanced features remain optional:

* Supervision — optional, for fault tolerance
* Compute pool — optional, for CPU-intensive work
* Timers — optional, for scheduled messages
* Resources/Capabilities — optional, for access control
* Tracing — optional, via `tracing-subscriber`

---

## 31. What Runact Should NOT Become

Runact should not attempt to become:

* a complete operating system
* a GUI toolkit
* an editor
* a web framework
* a database
* a distributed database
* an application framework

Runact is the concurrency/runtime foundation.

PaperOS is one major application built on top of it.

---

## 32. Distribution

Distributed actors may eventually be considered.

However, distribution is not part of the initial core.

First establish reliable local semantics:

```text
actor
message
mailbox
scheduler
supervision
compute
lifecycle
ownership
```

Only then consider:

```text
remote actors
remote messaging
cluster discovery
distributed supervision
```

---

## 33. Testing Strategy

Runact has extensive deterministic tests.

Test categories:

### Actor tests

* message processing (`tests/basic.rs`)
* state transitions
* lifecycle
* actor-to-actor communication

### Mailbox tests

* capacity (`tests/basic.rs` — `test_backpressure_rejects_when_full`)
* ordering
* backpressure
* concurrent producers

### Scheduler tests

* fairness
* work stealing (`tests/scheduler.rs`)
* yielding (reduction counting)

### Supervision tests

* panic (`supervisor_actor.rs` tests)
* restart (backoff, max restarts)
* stop
* escalation (beyond max restarts)

### Compute tests

* long-running tasks (`tests/compute.rs`)
* cancellation
* result delivery
* panic isolation

### Resource tests

* cleanup
* shutdown
* failure

### Stress tests

* thousands of actors (`benches/runact_bench.rs` — 100K actors)
* millions of messages
* high contention
* mailbox saturation

---

## 34. Performance Goals

Performance is measured rather than assumed.

Important benchmarks:

```text
actor spawn
message send
message throughput
context switching
scheduler latency
mailbox latency
request/reply latency
actor memory usage
work stealing
compute submission
shutdown
```

Benchmarks are implemented with Criterion in `benches/runact_bench.rs`.

The goal is a useful balance between:

```text
latency
throughput
fairness
memory
safety
simplicity
```

---

## 35. Architectural Invariants

These are explicit project rules.

### Invariant 1

Actor state belongs to the actor.

### Invariant 2

Messages are the primary actor communication mechanism.

### Invariant 3

Actors do not correspond to OS threads.

### Invariant 4

Long CPU work does not run on normal actor workers.

### Invariant 5

Actor failure is isolated whenever possible.

### Invariant 6

Resource ownership follows Rust ownership.

### Invariant 7

Backpressure must be explicit.

### Invariant 8

Cancellation must be cooperative for arbitrary Rust computation.

### Invariant 9

Runtime semantics must remain understandable without knowing internal implementation details.

### Invariant 10

The runtime must remain useful independently of PaperOS.

---

## 36. Development Order

Recommended implementation order (v1.0.0):

```text
1. Actor abstraction
2. Actor identity
3. Mailbox
4. Basic scheduler
5. Worker threads
6. Cooperative yielding
7. Work stealing
8. Request/reply
9. Lifecycle
10. Panic isolation
11. Supervision
12. Timers
13. Cancellation
14. Compute pool
15. Capabilities/resources
16. Tracing
17. Stress testing
18. Performance optimization
```

Do not add distributed actors or a complex plugin system before the local runtime semantics are stable.

---

## 37. Final Architectural Definition

Runact should be understood as:

> A Rust-native actor runtime that provides lightweight process isolation, message-driven concurrency, supervision, cooperative scheduling, and dedicated CPU execution while preserving Rust ownership and deterministic resource management.

Its most important feature is not any individual API.

Its most important feature is the programming model:

```text
State
  ↓
Actor
  ↓
Message
  ↓
Scheduler
  ↓
Actor
  ↓
Event / Result
```

Runact is the execution foundation.

PaperOS is a major consumer of that foundation.

---

## Dependencies

Minimal dependencies, each with a clear architectural reason:

```toml
[dependencies]
crossbeam-channel = "0.5"  # Mailboxes and inter-thread communication
serde = { version = "1", features = ["derive"] }  # Serialization for ActorId
tracing = "0.1"  # Observability
thiserror = "1"  # Error handling
```

**No Tokio.** Runact owns its runtime semantics.

## Source Layout

```text
runact/
├── Cargo.toml
├── README.md
├── CHANGELOG.md
├── LICENSE
├── docs/
│   ├── architecture.md
│   ├── actors.md
│   ├── actor-communication.md
│   ├── scheduler.md
│   ├── supervision.md
│   ├── runtime.md
│   ├── vision.md
│   ├── roadmap.md
│   ├── security.md
│   ├── resources.md
│   ├── extensions.md
│   ├── persistence.md
│   ├── transactions.md
│   ├── agents.md
│   ├── object-model.md
│   ├── protocol.md
│   ├── adr/
│   │   ├── 0000-template.md
│   │   ├── 0001-beam-style-scheduler.md
│   │   └── 0002-dual-scheduler-architecture.md
│   └── guides/
│       ├── getting-started.md
│       ├── supervision.md
│       ├── compute.md
│       ├── timers.md
│       └── resources.md
├── src/
│   ├── lib.rs
│   ├── actor/
│   ├── mailbox/
│   ├── scheduler/
│   ├── supervision/
│   ├── compute/
│   ├── timer/
│   ├── resource/
│   └── runtime.rs
├── examples/
│   └── counter.rs
├── benches/
│   └── runact_bench.rs
└── tests/
    ├── basic.rs
    ├── compute.rs
    ├── observability.rs
    ├── resource.rs
    ├── scheduler.rs
    └── timer.rs
```

## See Also

- [Actor Communication Principles](actor-communication.md) — 24 principles governing message flow, request/reply, backpressure, cancellation, and the fundamental invariant: actors must never block scheduler workers.
- [Actors](actors.md) — The Actor trait, ActorId, ActorContext, ownership model, message design.
- [Scheduler](scheduler.md) — BEAM-style scheduler with work stealing and reduction counting.
- [Supervision](supervision.md) — Supervision trees, restart strategies, backoff configuration.
- [Runtime](runtime.md) — Top-level Runtime API, RequestHandle, RuntimeConfig, RuntimeStats.
- [Getting Started Guide](guides/getting-started.md) — Walk through spawning actors, sending messages, and handling replies.
