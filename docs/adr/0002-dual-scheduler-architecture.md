# ADR-0002: Dual-Scheduler Architecture

## Status

Accepted

## Context

Runact needs to handle two distinct workloads:

1. **Lightweight actors** — Message passing, cooperative scheduling, millions of processes
2. **CPU-intensive tasks** — Parsing, compilation, encryption, long-running computations

A single scheduler cannot optimize for both. Actors need fast context switching and cooperative yield. Compute tasks need to run to completion without reduction counting overhead.

## Decision

Implement two separate schedulers with a typed result channel connecting them.

```
┌─────────────────────────────────────────────────────────┐
│                    Runtime                              │
├────────────────────────┬────────────────────────────────┤
│    Actor Scheduler     │     Compute Scheduler          │
│    (BEAM-style)        │     (Work-stealing thread pool)│
├────────────────────────┼────────────────────────────────┤
│ • Lightweight actors   │ • CPU-intensive tasks          │
│ • Message passing      │ • Long-running computations    │
│ • Reduction counting   │ • No reduction counting        │
│ • Cooperative yield    │ • Runs to completion           │
└───────────┬────────────┴────────────┬───────────────────┘
            │                         │
            │      Result channel     │
            └─────────────────────────┘
```

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Result channel | Oneshot | Compute job produces one result |
| Queue type | Bounded | Prevent unlimited memory growth |
| Actor waiting | Never block | Actor must remain responsive |
| Other messages | Continue processing | Core actor property |
| Panic handling | Catch at worker boundary | Convert panic into job failure |
| Timeout | Optional, no default | Cancellation/timeout semantics are tricky |
| Task interface | `FnOnce() -> T` | Simple, ergonomic, zero ceremony |
| Full queue | Reject | Never block an actor |
| Priority | Later | Useful but not needed for MVP |
| Cancellation | Cooperative | Arbitrary Rust code cannot safely be force-killed |

## Compute Scheduler Interface

```rust
pub struct ComputeScheduler {
    pool: ThreadPool,
    sender: crossbeam_channel::Sender<Task>,
    config: ComputeConfig,
}

pub struct ComputeConfig {
    pub max_workers: usize,
    pub queue_capacity: usize,
    pub default_timeout: Option<Duration>,
}

pub struct Task {
    pub id: TaskId,
    pub job: Box<dyn FnOnce() -> Box<dyn Any + Send> + Send>,
    pub result_sender: oneshot::Sender<ComputeResult>,
    pub created_at: Instant,
    pub timeout: Option<Duration>,
}

pub enum ComputeResult {
    Ok(Box<dyn Any + Send>),
    Err(ComputeError),
    Panic(String),
    Timeout,
    Rejected,
}

impl ComputeScheduler {
    pub fn new(config: ComputeConfig) -> Result<Self, ComputeError>;
    pub fn spawn<F, T>(&self, job: F) -> Result<ComputeHandle<T>, ComputeError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static;
    pub fn spawn_with_timeout<F, T>(&self, job: F, timeout: Duration) -> Result<ComputeHandle<T>, ComputeError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static;
    pub fn shutdown(self);
}

pub struct ComputeHandle<T> {
    receiver: oneshot::Receiver<ComputeResult>,
    _phantom: PhantomData<T>,
}

impl<T> ComputeHandle<T> {
    pub fn try_recv(&self) -> Option<Result<T, ComputeError>>;
    pub fn recv(self) -> Result<T, ComputeError>;
    pub fn recv_timeout(self, timeout: Duration) -> Result<T, ComputeError>;
}
```

## Actor Integration

From an actor's perspective:

```rust
struct MyActor {
    compute: ComputeScheduler,
}

impl Process for MyActor {
    type Message = ActorMessage;

    fn handle(&mut self, msg: Self::Message, ctx: &mut ProcessContext) -> Result<(), ProcessError> {
        match msg {
            ActorMessage::DoExpensiveWork(input) => {
                // Spawn compute task - does NOT block actor
                let handle = self.compute.spawn(move || {
                    expensive_computation(input)
                })?;

                // Actor continues processing other messages
                // Result arrives as a future message
                Ok(())
            }
            ActorMessage::ComputeResult(result) => {
                // Handle the result when it arrives
                match result {
                    Ok(value) => self.process_result(value),
                    Err(e) => self.handle_error(e),
                }
                Ok(())
            }
        }
    }
}
```

## Panic Handling

Compute workers catch panics at the boundary:

```rust
// Inside worker thread
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    (task.job)()
}));

match result {
    Ok(value) => task.result_sender.send(ComputeResult::Ok(Box::new(value))),
    Err(panic) => {
        let msg = if let Some(s) = panic.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic.downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        task.result_sender.send(ComputeResult::Panic(msg))
    }
}
```

## Backpressure

When compute queue is full:

```rust
impl ComputeScheduler {
    pub fn spawn<F, T>(&self, job: F) -> Result<ComputeHandle<T>, ComputeError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (result_sender, result_receiver) = oneshot::channel();

        let task = Task {
            id: TaskId::new(),
            job: Box::new(move || Box::new(job()) as Box<dyn Any + Send>),
            result_sender,
            created_at: Instant::now(),
            timeout: None,
        };

        // Try to send - reject if full, never block
        match self.sender.try_send(task) {
            Ok(()) => Ok(ComputeHandle::new(result_receiver)),
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                Err(ComputeError::QueueFull)
            }
            Err(crossbeam_channel::TrySendError::Closed(_)) => {
                Err(ComputeError::SchedulerShutdown)
            }
        }
    }
}
```

## Consequences

### Positive

- Actors never block on compute tasks
- Compute tasks can run as long as needed
- Clear separation of concerns
- Panic isolation between schedulers
- Backpressure prevents memory exhaustion

### Negative

- Two schedulers to maintain
- Result channel adds complexity
- Actor must handle async results explicitly
- No arbitrary cancellation of compute tasks

## References

- BEAM port processes
- Erlang NIFs
- Tokio blocking pool
- crossbeam-channel
