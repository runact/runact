# Scheduler

## Overview

Runact uses a BEAM-style scheduler with work stealing. The scheduler multiplexes many actors over a relatively small number of OS threads.

## Core Principle

> **Do NOT create one OS thread per actor.**

```text
100,000 Actors
       │
       ▼
Runact Scheduler
       │
 ┌─────┼─────┐
 ▼     ▼     ▼
Worker Worker Worker
```

The number of workers defaults to available CPU parallelism.

## Architecture

```text
┌─────────────────────────────────────────────────────────┐
│                    Scheduler                            │
├─────────────┬─────────────┬─────────────┬──────────────┤
│  Worker 0   │  Worker 1   │  Worker 2   │  Worker 3    │
│  ┌───────┐  │  ┌───────┐  │  ┌───────┐  │  ┌───────┐  │
│  │ RunQ  │  │  │ RunQ  │  │  │ RunQ  │  │  │ RunQ  │  │
│  │[A,B,C]│  │  │[D,E,F]│  │  │[G,H,I]│  │  │[J,K,L]│  │
│  └───────┘  │  └───────┘  │  └───────┘  │  └───────┘  │
│      │      │      │      │      │      │      │      │
│      ▼      │      ▼      │      ▼      │      ▼      │
│  Actor A    │  Actor D    │  Actor G    │  Actor J    │
│  (N reduc)  │  (N reduc)  │  (N reduc)  │  (N reduc)  │
└─────────────┴─────────────┴─────────────┴──────────────┘
                    │
            Work Stealing
```

## Components

### Scheduler

Top-level coordinator.

```rust
pub struct Scheduler {
    queues: Vec<RunQueue>,
    next_worker: AtomicUsize,
    handles: Vec<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}
```

- One `RunQueue` per worker
- Round-robin initial dispatch (`next_worker.fetch_add % queues.len()`)
- Atomic stop flag for shutdown

### Worker Loop

```rust
fn worker_loop(local_queue: RunQueue, steal_targets: Vec<RunQueue>, stop: Arc<AtomicBool>) {
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        // 1. Check own queue
        if let Some(task) = local_queue.pop() {
            task();
            continue;
        }

        // 2. Try to steal from other workers
        let mut stolen = false;
        for target in &steal_targets {
            let stolen_tasks = target.steal_half();
            if !stolen_tasks.is_empty() {
                for task in stolen_tasks {
                    local_queue.push(task);
                }
                stolen = true;
                break;
            }
        }

        // 3. If nothing to do, yield CPU
        if !stolen {
            thread::yield_now();
        }
    }
}
```

### RunQueue

Lock-based queue with work-stealing support.

```rust
pub struct RunQueue {
    queue: Arc<Mutex<VecDeque<Task>>>,
}
```

- `push` — adds task to back
- `pop` — removes task from front (local worker)
- `steal_half` — drains first half of queue (victim)

### ReductionCounter

Prevents starvation by counting message processing.

```rust
pub const MAX_REDUCTIONS: u64 = 4000;

pub struct ReductionCounter {
    count: AtomicU64,
}
```

After each message is processed by an actor, the counter is incremented. When it reaches `MAX_REDUCTIONS`, the counter is reset and `thread::yield_now()` is called to give other workers a chance to run.

## Algorithm

### Basic Flow

```text
1. Actor spawns → task added to worker queue (round-robin)
2. Worker picks task from own queue (pop_front)
3. Actor processes message
4. Reduction counter increments
5. If counter >= MAX_REDUCTIONS → yield (thread::yield_now)
6. Otherwise → continue with next message from mailbox
7. If no work → try to steal from other workers
8. If nothing to steal → thread::yield_now
```

### Work Stealing

```text
Worker 0 idle
    │
    ▼
Check own queue → empty
    │
    ▼
Try to steal from other workers
    │
    ├── Found work → execute stolen tasks
    │
    └── No work → yield → retry
```

The `steal_half()` method takes half of the victim's queue (rounded down), moving tasks to the stealing worker's local queue.

## Configuration

### Default Settings

```rust
let scheduler = Scheduler::new();
```

- Workers = available parallelism (usually CPU cores)
- Queue capacity = unbounded
- Reduction limit = 4000

### Worker Count

```rust
pub fn worker_count(&self) -> usize {
    self.queues.len()
}
```

## Cooperative Scheduling

### Why Cooperative?

Rust code cannot safely be arbitrarily preempted at any instruction boundary. Runact uses cooperative scheduling at safe execution boundaries — after each message is processed, the actor handler returns and the scheduler decides whether to continue or yield.

### How It Works

The actor's message loop (inside `Runtime::spawn`):

```rust
loop {
    match rx.recv_timeout(Duration::from_millis(10)) {
        Ok(envelope) => {
            // Process message...
            ctx.set_reply_sender(None);
            let _ = actor.handle(*msg, &mut ctx);

            // Cooperative yield after MAX_REDUCTIONS messages
            if reductions.increment() >= MAX_REDUCTIONS {
                reductions.reset();
                std::thread::yield_now();
            }
        }
        Err(RecvTimeoutError::Timeout) => {
            reductions.reset();
            continue;
        }
        Err(RecvTimeoutError::Disconnected) => break,
    }
}
```

### What Actors Should Do

- Process one message
- Perform bounded work
- Return quickly
- Don't loop forever inside a single handler invocation
- Don't block on I/O (use timers or compute pool)

### What Actors Should NOT Do

- Run for milliseconds without yielding
- Block on external resources
- Spin in loops
- Call `thread::sleep`
- Perform CPU-intensive work (use `ctx.spawn_compute` instead)

## Monitoring

### Metrics

The runtime exposes basic observability via `tracing` and `RuntimeStats`:

```rust
pub struct RuntimeStats {
    pub actor_count: usize,
    pub request_count: u64,
}
```

### Observability

```rust
// Trace actor execution
tracing::info!(actor_id = %id, "spawned actor");
tracing::debug!(target_id = %target, "send");
tracing::trace!(actor_id = %id, "handling message");
```

### Logging Levels

| Level | Events |
|-------|--------|
| `info` | Actor spawned, actor stopped, runtime shutdown |
| `debug` | Message sent, request sent |
| `trace` | Per-message handling |
| `warn` | Actor restarted (via supervisor) |
| `error` | Max restarts exceeded, actor failures |

## Development Phases

### Phase 1 — Runtime Core (v1.0.0)

- ✅ `src/actor/` — ActorId, Actor trait, ActorContext
- ✅ `src/mailbox/` — Mailbox with backpressure policies
- ✅ `src/scheduler/` — BEAM-style scheduler with work stealing
- ✅ `src/runtime.rs` — Top-level coordinator
- ✅ Message passing (fire-and-forget, request/reply)
- ✅ Actor-to-actor communication
- ✅ Graceful shutdown
- ✅ Reduction counting (cooperative preemption)

### Phase 2 — Reliability

- ✅ `src/supervision/` — Supervisor, ChildSpec, RestartStrategy
- ✅ One-for-one restart strategy with exponential backoff
- ✅ Crash isolation (compute tasks)
- ✅ Restart limits

### Phase 3 — Compute

- ✅ `src/compute/` — ComputeScheduler, ComputeHandle
- ✅ Bounded queue
- ✅ Task submission from actors
- ✅ Panic isolation

### Future Phases

- Timers (✅ v1.0.0)
- Cancellation (compute task cancellation, ✅ v1.0.0)
- Resources and capabilities (✅ v1.0.0)
- Tracing (✅ v1.0.0)
- Stress testing and performance optimization
