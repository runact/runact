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
└─────────────┴─────────────┴─────────────┴──────────────┘
                    │
            Work Stealing
```

## Components

### Scheduler

Top-level coordinator.

```rust
pub struct Scheduler {
    workers: Vec<Worker>,
    sender: Option<Sender<Task>>,
    handles: Vec<JoinHandle<()>>,
}
```

### Worker

Per-core scheduler with own run queue.

```rust
struct Worker {
    id: usize,
    queue: Arc<Mutex<VecDeque<Task>>>,
}
```

### RunQueue

Lock-free MPSC queue per worker.

```rust
struct RunQueue {
    queue: Arc<Mutex<VecDeque<Task>>>,
}
```

### ReductionCounter

Prevents starvation by counting message processing.

```rust
pub const MAX_REDUCTIONS: u64 = 4000;

struct ReductionCounter {
    count: AtomicU64,
}
```

## Algorithm

### Basic Flow

```text
1. Actor spawns → added to worker queue
2. Worker picks actor from queue
3. Actor processes one message
4. Reduction counter increments
5. If counter >= MAX_REDUCTIONS → preempt
6. Otherwise → continue with next message
7. If no work → try to steal from other workers
```

### Work Stealing

```text
Worker 0 idle
    │
    ▼
Check own queue → empty
    │
    ▼
Try to steal from random worker
    │
    ├── Found work → execute
    │
    └── No work → sleep briefly → retry
```

## Priorities

The scheduler should prioritize:

1. **Responsiveness** — Actors respond quickly
2. **Fairness** — No actor starves
3. **Throughput** — Process many messages
4. **Predictability** — Consistent behavior

## Implementation

### Spawning an Actor

```rust
impl Scheduler {
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if let Some(sender) = &self.sender {
            let _ = sender.send(Box::new(f));
        }
    }
}
```

### Worker Loop

```rust
fn worker_loop(id: usize, queue: Arc<Mutex<VecDeque<Task>>>) {
    loop {
        let task = {
            let mut q = queue.lock().unwrap();
            q.pop_front()
        };

        match task {
            Some(task) => {
                task();
            }
            None => {
                // No work available, yield
                thread::yield_now();
            }
        }
    }
}
```

## Configuration

### Default Settings

```rust
let scheduler = Scheduler::new();
```

- Workers = available parallelism (usually CPU cores)
- Queue capacity = unbounded initially
- Reduction limit = 4000

### Custom Settings

```rust
let scheduler = Scheduler::with_config(SchedulerConfig {
    num_workers: 8,
    max_reductions: 2000,
});
```

## Cooperative Scheduling

### Why Cooperative?

Rust code cannot safely be arbitrarily preempted at any instruction boundary. Runact uses cooperative scheduling at safe execution boundaries.

### How It Works

```rust
fn handle(&mut self, msg: Message, ctx: &mut ActorContext) -> Result<(), ActorError> {
    // Process one message
    self.process(msg);
    
    // Yield back to scheduler
    // (Implicit - handler returns)
    
    Ok(())
}
```

### What Actors Should Do

- Process one message
- Perform bounded work
- Return quickly
- Don't loop forever
- Don't block on I/O

### What Actors Should NOT Do

- Run for milliseconds
- Block on external resources
- Spin in loops
- Call `thread::sleep`
- Perform CPU-intensive work (use compute pool)

## Monitoring

### Metrics

```rust
pub struct SchedulerMetrics {
    pub worker_count: usize,
    pub total_actors: usize,
    pub messages_processed: u64,
    pub steals_attempted: u64,
    pub steals_succeeded: u64,
}
```

### Observability

```rust
// Trace actor execution
tracing::debug!(
    actor_id = %actor_id,
    message_count = count,
    duration_ms = elapsed.as_millis(),
    "Actor executed"
);
```

## Future Enhancements

### Priority Scheduling

```rust
enum Priority {
    Critical,
    High,
    Normal,
    Low,
}
```

### Actor Affinity

```rust
// Pin actor to specific worker
scheduler.spawn_with_affinity(actor, worker_id);
```

### Work Stealing Strategies

- Random victim
- Least loaded victim
- Nearest neighbor
