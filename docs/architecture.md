# Runact Architecture

## Overview

Runact is a single Rust crate that provides a BEAM-style actor runtime. The runtime combines lightweight processes, messaging, scheduling, and supervision with Rust's ownership model.

## Core Definition

> **Runact is a Rust-native actor runtime that combines BEAM-style lightweight processes, messaging, scheduling and supervision with Rust's ownership model, providing the foundation for a programmable editor-centric computing environment.**

## Project Structure

```
runact/
├── Cargo.toml
├── README.md
├── LICENSE
├── docs/
│   ├── architecture.md
│   ├── actors.md
│   ├── actor-communication.md
│   ├── scheduler.md
│   ├── supervision.md
│   └── roadmap.md
│
├── src/
│   ├── lib.rs
│   ├── actor/
│   │   ├── mod.rs
│   │   ├── id.rs          # ActorId
│   │   ├── actor.rs       # Actor trait
│   │   ├── context.rs     # ActorContext
│   │   └── error.rs       # ActorError
│   ├── scheduler/
│   │   ├── mod.rs
│   │   ├── scheduler.rs   # BEAM-style scheduler
│   │   ├── worker.rs      # Per-core worker
│   │   ├── run_queue.rs   # Lock-free queue
│   │   └── reduction.rs   # Reduction counter
│   ├── mailbox/
│   │   ├── mod.rs
│   │   └── mailbox.rs     # crossbeam-channel wrapper
│   ├── supervision/
│   │   ├── mod.rs
│   │   ├── supervisor.rs  # Supervisor
│   │   ├── strategy.rs    # Restart strategies
│   │   └── child.rs       # Child specification
│   ├── compute/
│   │   ├── mod.rs
│   │   ├── scheduler.rs   # Compute pool
│   │   ├── task.rs        # Task abstraction
│   │   └── handle.rs      # ComputeHandle
│   ├── timer/
│   │   ├── mod.rs
│   │   └── timer.rs       # Timer wheel
│   └── runtime.rs         # Top-level coordinator
│
└── examples/
    ├── counter.rs
    └── supervisor.rs
```

## Layer Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Applications                      │
│         TUI │ GUI │ CLI │ Web │ Editor │ Server     │
└─────────────────────┬───────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────┐
│               Runtime Layer                         │
│     Runtime (top-level coordinator)                 │
└─────────────────────┬───────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────┐
│              Core Components                        │
├─────────────────────┬───────────────────────────────┤
│    Actor System     │      Compute System           │
├─────────────────────┼───────────────────────────────┤
│ • Actor trait       │ • Compute pool                │
│ • ActorId           │ • Task submission             │
│ • Mailbox           │ • Oneshot result              │
│ • Supervision       │ • Panic isolation             │
│ • Timers            │ • Bounded queue               │
└───────────┬─────────┴─────────────┬─────────────────┘
            │                       │
┌───────────▼───────────────────────▼─────────────────┐
│              Scheduler Layer                        │
├─────────────────────────────────────────────────────┤
│    BEAM-style Scheduler with Work Stealing          │
├─────────────┬─────────────┬─────────────┬───────────┤
│   Worker 0  │   Worker 1  │   Worker 2  │  Worker 3 │
└─────────────┴─────────────┴─────────────┴───────────┘
                      │
┌─────────────────────▼───────────────────────────────┐
│              Platform Layer                         │
│     Threads (OS) │ Atomics │ crossbeam-channel      │
└─────────────────────────────────────────────────────┘
```

## Core Components

### Actor

The fundamental unit of computation.

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

### ActorId

Stable, unique identifier for an actor.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActorId(u64);
```

### Mailbox

Typed message queue for an actor.

- Uses `crossbeam-channel` internally
- Hidden behind abstraction
- Supports bounded capacity
- Ownership transfer on send

### Scheduler

BEAM-style scheduler with work stealing.

- Per-core worker threads
- Lock-free run queues
- Work stealing between workers
- Reduction counting for fairness

### Compute Pool

Separate execution domain for CPU-intensive tasks.

- Bounded queue
- Oneshot result channel
- Panic isolation
- Never blocks actors

### Supervision

BEAM-inspired supervision tree.

- One-for-one restart strategy
- Child specifications
- Backoff configuration
- Failure propagation

### Timers

Scheduled message delivery.

- One-shot timers
- Periodic timers
- Integrated with scheduler

## Design Principles

### 1. Ownership is Fundamental

```rust
// Actor owns its state
struct MyActor {
    data: Vec<String>,  // Owned by this actor
}

// Communication transfers ownership
actor.send(MyMessage { data });  // Ownership moves
```

### 2. No Shared Mutable State

```rust
// ❌ FORBIDDEN
let shared = Arc<Mutex<State>>();

// ✅ CORRECT
// Each actor owns its state
// Communication via messages
```

### 3. Implementation Details Hidden

```rust
// ✅ Public API
actor.send(message);

// ❌ Not exposed
crossbeam_sender.send(message);
```

### 4. Cooperative Scheduling

```rust
fn handle(&mut self, msg: Message, ctx: &mut ActorContext) -> Result<(), ActorError> {
    // Process one message
    // Yield back to scheduler
    // Don't monopolize worker
    Ok(())
}
```

## Dependencies

Minimal dependencies, each with clear architectural reason:

```toml
[dependencies]
crossbeam-channel = "0.5"  # Mailbox implementation
serde = { version = "1", features = ["derive"] }  # Serialization
tracing = "0.1"  # Observability
thiserror = "1"  # Error handling
```

**No Tokio.** Runact owns its runtime semantics.

## Development Phases

### Phase 1 — Runtime Core
- ActorId, Actor trait
- Mailbox, Scheduler
- Actor spawning, messaging
- Lifecycle, graceful shutdown

### Phase 2 — Reliability
- Panic isolation
- Supervisors, restart strategies
- Failure propagation

### Phase 3 — Compute
- Bounded compute pool
- Task submission, oneshot result
- Panic isolation

### Phase 4 — Timers and I/O
- Timers
- Filesystem, process spawning
- Basic networking

### Phase 5 — Resources and Capabilities
- Resource handles
- Capability-oriented access

### Phase 6 — Editor Runtime
- First minimal editor on Runact

### Phase 7 — Programmability
- Extension API
- Scripting integration

## First Demonstration

```text
Runtime
│
├── CounterActor
├── LoggerActor
├── ComputeActor
└── Supervisor
```

Demonstrate:
1. Spawning actors
2. Sending messages
3. Actor state isolation
4. Scheduler execution
5. Actor failure
6. Supervisor restart
7. Compute task execution
8. Graceful shutdown

## See Also

- [Actor Communication Principles](actor-communication.md) — 24 principles governing message flow, request/reply, backpressure, cancellation, and the fundamental invariant: actors must never block scheduler workers.
