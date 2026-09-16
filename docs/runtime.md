# Runact Runtime

## Overview

The Runact runtime is the execution kernel. It manages processes (actors), message delivery, scheduling, supervision, and timers. Uses a custom BEAM-style scheduler with work stealing. No Tokio types leak through public APIs.

## Core Concepts

### Actor

An Actor is an independent unit of computation with:

- Stable `ActorId`
- Owns mutable state
- Receives messages sequentially
- Processes one message at a time
- Can send messages to other actors
- Can spawn child actors (via supervisor)
- Can be supervised
- Can schedule timers
- Can submit compute tasks
- Can reply to requests

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

A stable, unique identifier for an actor.

- Created at spawn time
- Never reused within a runtime instance
- Serializable (via `serde`)
- Orderable (for debugging)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ActorId(u64);
```

### ActorContext

Provided to actor handlers.

```rust
pub struct ActorContext {
    actor_id: ActorId,
    _sender: crossbeam_channel::Sender<MessageEnvelope>,
    senders: Option<SenderMap>,
    reply_sender: Option<ReplySender>,
    compute_sender: Option<crossbeam_channel::Sender<Task>>,
    timer_handle: Option<TimerHandle>,
}
```

### Capabilities

- `actor_id()` — Get own ID
- `send_to(target, msg)` — Send fire-and-forget message to another actor
- `reply(msg)` — Reply to a request
- `is_request()` — Check if current message expects a reply
- `spawn_compute(job)` — Submit CPU-intensive task
- `schedule_timer(dur, msg)` — One-shot timer
- `schedule_interval(interval, msg)` — Periodic timer
- `cancel_timer(timer_id)` — Cancel a timer

### Lifecycle

```text
spawned → running → stopping → stopped
    │                  │
    │                  └──→ terminated
    │
    └──→ failed → restarting → running
```

States:

- **spawned** — Actor created, not yet scheduled
- **running** — Actor is executing
- **stopping** — Graceful shutdown initiated
- **stopped** — Actor completed normally
- **terminated** — Actor was forcibly stopped
- **failed** — Actor crashed or returned error
- **restarting** — Supervisor is restarting the actor

## Mailbox

Each actor has a mailbox with bounded capacity.

### Requirements

- FIFO ordering within an actor
- Configurable capacity (default: 1000)
- Backpressure policy: Reject (default) or Block
- Shutdown behavior: drain remaining messages

### Backpressure Strategies

When the mailbox is full:

| Strategy | Behavior |
|----------|----------|
| `Reject` | Return `RuntimeError::MailboxFull` to sender |
| `Block` | Sender waits until space available (`send_blocking`) |

### BackpressurePolicy (Internal)

```rust
pub(crate) enum BackpressurePolicy {
    Reject,
    Block,
    DropLowPriority,
}
```

### Mailbox Interface

The actual mailbox used by the runtime is `crossbeam_channel::bounded`:

```rust
let (tx, rx) = crossbeam_channel::bounded::<MessageEnvelope>(self.mailbox_capacity);
```

### Metrics

Mailbox depth is exposed via `ActorInfo`:

```rust
pub struct ActorInfo {
    pub id: ActorId,
    pub name: Option<String>,
    pub mailbox_depth: usize,
}
```

## Scheduler

BEAM-style scheduler with work stealing.

### Architecture

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

### Requirements

- Lightweight actor creation
- Work stealing across cores
- Per-worker run queues
- Cooperative scheduling with reduction counting
- Cancellation (via stop flag)
- Timers (via TimerService)
- Process lifecycle management

### Scheduler Interface

```rust
pub struct Scheduler {
    queues: Vec<RunQueue>,
    next_worker: AtomicUsize,
    handles: Vec<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl Scheduler {
    pub fn new() -> Self;
    pub fn spawn<F>(&self, f: F) where F: FnOnce() + Send + 'static;
    pub fn shutdown(&mut self);
    pub fn worker_count(&self) -> usize;
}
```

Workers spawn at `new()` and default to `available_parallelism()` count. The round-robin dispatch uses `next_worker.fetch_add(1, Ordering::Relaxed) % queues.len()`.

### ReductionCounter

```rust
pub const MAX_REDUCTIONS: u64 = 4000;

pub struct ReductionCounter {
    count: AtomicU64,
}

impl ReductionCounter {
    pub fn new() -> Self;
    pub fn increment(&self) -> u64;
    pub fn reset(&self);
}
```

After each message is processed, the counter is incremented. When it reaches `MAX_REDUCTIONS`, the counter is reset and `thread::yield_now()` is called.

## Compute Scheduler

For CPU-intensive tasks that would starve actors.

### Architecture

```text
┌─────────────────────────────────────────────────────────┐
│                 Compute Scheduler                       │
├─────────────┬─────────────┬─────────────┬──────────────┤
│  Worker 0   │  Worker 1   │  Worker 2   │  Worker 3    │
│  ┌───────┐  │  ┌───────┐  │  ┌───────┐  │  ┌───────┐  │
│  │ Task  │  │  │ Task  │  │  │ Task  │  │  │ Task  │  │
│  │[X,Y,Z]│  │  │[P,Q,R]│  │  │[S,T,U]│  │  │[V,W,X]│  │
│  └───────┘  │  └───────┘  │  └───────┘  │  └───────┘  │
│      │      │      │      │      │      │      │      │
│      ▼      │      ▼      │      ▼      │      ▼      │
│  Run to     │  Run to     │  Run to     │  Run to     │
│  completion │  completion │  completion │  completion │
└─────────────┴─────────────┴─────────────┴──────────────┘
```

### Requirements

- CPU-intensive tasks run to completion
- Work stealing across cores
- Bounded queue for backpressure (soft limit via `queue_capacity` config)
- Panic isolation at worker boundary
- Oneshot result channel
- Never block actors

### Compute Scheduler Interface

```rust
pub struct ComputeScheduler {
    sender: Option<crossbeam_channel::Sender<Task>>,
    handles: Vec<JoinHandle<()>>,
    _config: ComputeConfig,
    stop: Arc<AtomicBool>,
}

pub struct ComputeConfig {
    pub max_workers: usize,        // Defaults to available parallelism
    pub queue_capacity: usize,     // Soft limit (default: 1024)
    pub task_timeout: Option<Duration>,  // Reserved for future use
}

impl ComputeScheduler {
    pub fn new(config: ComputeConfig) -> Result<Self, ComputeError>;
    pub fn spawn<F, T>(&self, job: F) -> Result<ComputeHandle<T>, ComputeError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static;
    pub fn sender(&self) -> Option<crossbeam_channel::Sender<Task>>;
    pub fn shutdown(&mut self);
}
```

### Task

```rust
pub struct Task {
    pub id: TaskId,
    pub job: Box<dyn FnOnce() -> Box<dyn Any + Send> + Send>,
    pub result_sender: crossbeam_channel::Sender<ComputeResult>,
    pub created_at: Instant,
    pub cancelled: Arc<AtomicBool>,
}
```

### ComputeResult

```rust
pub enum ComputeResult {
    Ok(Box<dyn Any + Send>),
    Err(ComputeError),
    Panic(String),
    Timeout,        // Reserved for future use
    Cancelled,
}
```

### ComputeHandle

```rust
pub struct ComputeHandle<T> {
    receiver: crossbeam_channel::Receiver<ComputeResult>,
    cancelled: Arc<AtomicBool>,
    _phantom: PhantomData<T>,
}

impl<T> ComputeHandle<T> {
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
    pub fn try_recv(&self) -> Option<Result<T, ComputeError>> where T: 'static;
    pub fn recv(self) -> Result<T, ComputeError> where T: 'static;
    pub fn recv_timeout(self, timeout: Duration) -> Result<T, ComputeError> where T: 'static;
}
```

### Panic Handling

Workers catch panics at the boundary:

```rust
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (task.job)()));
```

Panics are surfaced as `ComputeResult::Panic(msg)` and then as `ComputeError::WorkerPanic(msg)`.

### Backpressure

When the compute queue is full (unbounded channel in current implementation — queue_capacity is a soft limit not yet enforced), tasks are still accepted. The `ComputeError::QueueFull` error variant exists for future bounded queue support.

## Timer

Scheduled message delivery.

### Types

- **One-shot** — Fire once after delay
- **Periodic** — Fire repeatedly at interval (with drift correction)

### Timer Interface

```rust
pub struct TimerService {
    timers: Arc<Mutex<Vec<TimerEntry>>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

pub struct TimerHandle {
    timers: Arc<Mutex<Vec<TimerEntry>>>,
}

impl TimerHandle {
    pub fn schedule_timer<M: Send + 'static>(&self, duration: Duration, actor_id: ActorId, message: M) -> TimerId;
    pub fn schedule_interval<M: Clone + Send + Sync + 'static>(&self, interval: Duration, actor_id: ActorId, message: M) -> TimerId;
    pub fn cancel_timer(&self, timer_id: TimerId);
}
```

### Drift Correction

Periodic timers use scheduled-at-based rescheduling:

```rust
// Reschedule based on previous scheduled time
scheduled_at: entry_scheduled_at + interval
```

This prevents drift accumulation.

## Cancellation

Cooperative cancellation via tokens.

### CancellationToken (via ComputeHandle)

```rust
pub fn cancel(&self) {
    self.cancelled.store(true, Ordering::Relaxed);
}

pub fn is_cancelled(&self) -> bool {
    self.cancelled.load(Ordering::Relaxed)
}
```

The compute worker checks the flag before and after execution. If set before, the task is skipped. If set after, the result is discarded.

## Runtime

Top-level coordinator that owns all components.

### Runtime Interface

```rust
pub struct Runtime {
    scheduler: Scheduler,
    compute: ComputeScheduler,
    timers: TimerService,
    actors: Arc<RwLock<HashMap<ActorId, ActorInfo>>>,
    senders: Arc<RwLock<HashMap<ActorId, crossbeam_channel::Sender<MessageEnvelope>>>>,
    request_counter: AtomicU64,
    stop: Arc<AtomicBool>,
    mailbox_capacity: usize,
    shutdown_timeout: Duration,
}

impl Runtime {
    pub fn new() -> Result<Self, RuntimeError>;
    pub fn with_config(config: RuntimeConfig) -> Result<Self, RuntimeError>;
    pub fn spawn<A: Actor>(&mut self, actor: A) -> Result<ActorId, RuntimeError>;
    pub fn send<M: Send + 'static>(&self, target: ActorId, message: M) -> Result<(), RuntimeError>;
    pub fn send_blocking<M: Send + 'static>(&self, target: ActorId, message: M) -> Result<(), RuntimeError>;
    pub fn request<M: Send + 'static>(&self, target: ActorId, message: M) -> Result<RequestHandle, RuntimeError>;
    pub fn request_blocking<M: Send + 'static, R: Send + 'static>(&self, target: ActorId, message: M, timeout: Option<Duration>) -> Result<R, RuntimeError>;
    pub fn spawn_supervisor(&self, strategy: RestartStrategy, children: Vec<ChildSpec>) -> Result<ActorId, RuntimeError>;
    pub fn list_actors(&self) -> Result<Vec<ActorInfo>, RuntimeError>;
    pub fn inspect_actor(&self, id: ActorId) -> Result<Option<ActorInfo>, RuntimeError>;
    pub fn compute(&self) -> &ComputeScheduler;
    pub fn schedule_timer<M: Send + 'static>(&self, duration: Duration, target: ActorId, message: M) -> TimerId;
    pub fn schedule_interval<M: Clone + Send + Sync + 'static>(&self, interval: Duration, target: ActorId, message: M) -> TimerId;
    pub fn cancel_timer(&self, timer_id: TimerId);
    pub fn stats(&self) -> RuntimeStats;
    pub fn shutdown(&mut self) -> Result<(), RuntimeError>;
}
```

### RequestHandle

```rust
pub struct RequestHandle {
    id: u64,
    receiver: crossbeam_channel::Receiver<Box<dyn std::any::Any + Send>>,
}

impl RequestHandle {
    pub fn id(&self) -> u64;
    pub fn recv(self) -> Result<Box<dyn std::any::Any + Send>, RuntimeError>;
    pub fn try_recv(&self) -> Result<Box<dyn std::any::Any + Send>, RuntimeError>;
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Box<dyn std::any::Any + Send>, RuntimeError>;
}
```

### RuntimeConfig

```rust
pub struct RuntimeConfig {
    pub compute: ComputeConfig,
    pub mailbox_capacity: usize,       // Default: 1000
    pub shutdown_timeout: Duration,   // Default: 5 seconds
}
```

### RuntimeStats

```rust
pub struct RuntimeStats {
    pub actor_count: usize,
    pub request_count: u64,
}
```

### Shutdown

Graceful shutdown:

1. Sets the stop flag (actors stop accepting new messages and drain remaining)
2. Waits up to `shutdown_timeout` for actors to finish
3. Clears all internal state
4. Shuts down scheduler, timers, and compute pool

```rust
impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
```

## Supervisor

Manages child actor lifecycle and restart policies.

### Restart Strategy

```rust
pub enum RestartStrategy {
    OneForOne {
        max_restarts: usize,
        within: Duration,
        base_backoff: Duration,
    },
}
```

### Child Specification

```rust
pub struct ChildSpec {
    pub name: String,
    pub restart_policy: RestartPolicy,
    pub shutdown_timeout: Duration,
}

pub enum RestartPolicy {
    Permanent,   // Always restart
    Temporary,   // Never restart
    Transient,   // Restart only on abnormal exit
}
```

## Exit Criterion

Phase 1 (v1.0.0) is complete when:

- 100K+ concurrent actors communicate reliably
- Actor creation is lightweight
- Message delivery is reliable
- Work stealing across cores
- No starvation (reduction counting works)
- Timers fire correctly
- Cancellation works
- Shutdown is graceful
- Metrics are available
