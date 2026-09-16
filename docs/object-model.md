# Runact Object Model

## Overview

This document defines the core runtime objects in Runact. These objects form the public API surface. Implementation details do not leak through these types.

## Core Identifiers

### ActorId

Stable, unique identifier for an actor.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ActorId(u64);
```

- Created at spawn time
- Never reused within a runtime instance
- Serializable for persistence (via `serde`)
- Orderable for debugging
- Display format: `Actor(42)`

### RequestHandle

Handle to a pending request. Receives the reply asynchronously.

```rust
pub struct RequestHandle {
    id: u64,
    receiver: crossbeam_channel::Receiver<Box<dyn std::any::Any + Send>>,
}
```

Methods:

- `id()` — Get the request's correlation ID
- `recv()` — Block until reply arrives
- `try_recv()` — Return immediately — `Ok` or `Err`
- `recv_timeout(dur)` — Block up to `dur`, then return error

---

## Actor

### Actor Trait

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

### ActorContext

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

Methods:

- `actor_id()` — Returns the actor's own `ActorId`
- `send_to(target, msg)` — Send a fire-and-forget message to another actor
- `reply(msg)` — Reply to the sender of a request (no-op for fire-and-forget)
- `is_request()` — Check if the current message was sent as a request
- `spawn_compute(job)` — Submit a CPU-intensive task to the compute pool
- `schedule_timer(dur, msg)` — Schedule a one-shot timer
- `schedule_interval(interval, msg)` — Schedule a periodic timer
- `cancel_timer(timer_id)` — Cancel a previously scheduled timer

### ActorError

```rust
pub enum ActorError {
    Handler(String),
    Cancelled,
    Timeout,
    Panic(String),
}
```

---

## ActorInfo

Public view of a spawned actor.

```rust
pub struct ActorInfo {
    pub id: ActorId,
    pub name: Option<String>,
    pub mailbox_depth: usize,
}
```

---

## MessageEnvelope

Internal envelope wrapping messages sent to actors. Contains either a regular message or a request with a reply channel.

```rust
pub(crate) enum MessageEnvelope {
    Message(Box<dyn std::any::Any + Send>),
    Request {
        _request_id: u64,
        payload: Box<dyn std::any::Any + Send>,
        reply_sender: crossbeam_channel::Sender<Box<dyn std::any::Any + Send>>,
    },
}
```

---

## Mailbox

### BackpressurePolicy (Internal)

```rust
pub(crate) enum BackpressurePolicy {
    Reject,
    Block,
    DropLowPriority,
}
```

### MailboxConfig (Internal)

```rust
pub(crate) struct MailboxConfig {
    pub capacity: usize,
    pub backpressure: BackpressurePolicy,
}
```

Default: `capacity = 1000`, `backpressure = Reject`

---

## Scheduler

### Scheduler

```rust
pub struct Scheduler {
    queues: Vec<RunQueue>,
    next_worker: AtomicUsize,
    handles: Vec<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}
```

### RunQueue

```rust
pub struct RunQueue {
    queue: Arc<Mutex<VecDeque<Task>>>,
}
```

### ReductionCounter

```rust
pub const MAX_REDUCTIONS: u64 = 4000;

pub struct ReductionCounter {
    count: AtomicU64,
}
```

---

## Compute Scheduler

### ComputeConfig

```rust
pub struct ComputeConfig {
    pub max_workers: usize,
    pub queue_capacity: usize,
    pub task_timeout: Option<Duration>,
}
```

### ComputeScheduler

```rust
pub struct ComputeScheduler {
    sender: Option<crossbeam_channel::Sender<Task>>,
    handles: Vec<JoinHandle<()>>,
    _config: ComputeConfig,
    stop: Arc<AtomicBool>,
}
```

### ComputeHandle

```rust
pub struct ComputeHandle<T> {
    receiver: crossbeam_channel::Receiver<ComputeResult>,
    cancelled: Arc<AtomicBool>,
    _phantom: PhantomData<T>,
}
```

Methods:

- `cancel()` — Request cancellation
- `is_cancelled()` — Check if cancellation was requested
- `try_recv()` — Non-blocking poll
- `recv()` — Block until result arrives
- `recv_timeout(dur)` — Block up to timeout

### ComputeError

```rust
pub enum ComputeError {
    QueueFull,
    SchedulerShutdown,
    WorkerPanic(String),
}
```

### ComputeResult

```rust
pub enum ComputeResult {
    Ok(Box<dyn Any + Send>),
    Err(ComputeError),
    Panic(String),
    Timeout,
    Cancelled,
}
```

---

## Timer

### TimerId

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);
```

### TimerHandle

```rust
pub struct TimerHandle {
    timers: Arc<Mutex<Vec<TimerEntry>>>,
}
```

Methods:

- `schedule_timer(dur, actor_id, msg)` — One-shot timer
- `schedule_interval(interval, actor_id, msg)` — Periodic timer
- `cancel_timer(timer_id)` — Cancel a timer

### TimerConfig

```rust
pub struct TimerConfig {
    pub id: TimerId,
    pub actor_id: ActorId,
    pub scheduled_at: Instant,
    pub message: Box<dyn Any + Send>,
    pub interval: Option<Duration>,
}
```

### Timer

```rust
pub struct Timer {
    id: TimerId,
    actor_id: ActorId,
    scheduled_at: Instant,
    _message: Box<dyn Any + Send>,
    interval: Option<Duration>,
}
```

---

## Resource

### ResourceHandle

```rust
pub trait ResourceHandle: Any + Send + Sync + fmt::Debug + 'static {
    fn resource_type(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}
```

### Capability

```rust
pub struct Capability<H: ResourceHandle> {
    owner: ActorId,
    handle: Arc<H>,
}
```

Methods:

- `new(owner, handle)` — Create a new capability
- `owner()` — Returns the owning `ActorId`
- `handle()` — Borrows the underlying resource handle
- `into_handle()` — Consumes and returns the shared handle

### ResourceRegistry

```rust
pub struct ResourceRegistry {
    resources: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}
```

Methods:

- `new()` — Create empty registry
- `register(cap)` — Register a capability
- `get()` — Retrieve by type
- `remove()` — Remove and return by type
- `has()` — Check existence by type

---

## Runtime

### RuntimeConfig

```rust
pub struct RuntimeConfig {
    pub compute: ComputeConfig,
    pub mailbox_capacity: usize,    // Default: 1000
    pub shutdown_timeout: Duration, // Default: 5 seconds
}
```

### RuntimeStats

```rust
pub struct RuntimeStats {
    pub actor_count: usize,
    pub request_count: u64,
}
```

### RuntimeError

```rust
pub enum RuntimeError {
    ActorNotFound(ActorId),
    MailboxFull(ActorId),
    ActorExists(ActorId),
    ShutdownTimeout,
    RuntimeStopped,
}
```

---

## Supervision

### RestartStrategy

```rust
pub enum RestartStrategy {
    OneForOne {
        max_restarts: usize,
        within: Duration,
        base_backoff: Duration,
    },
}
```

### ChildSpec

```rust
pub struct ChildSpec {
    pub name: String,
    pub restart_policy: RestartPolicy,
    pub shutdown_timeout: Duration,
}
```

### RestartPolicy

```rust
pub enum RestartPolicy {
    Permanent,   // Always restart
    Temporary,   // Never restart
    Transient,   // Restart only on abnormal exit
}
```

---

## Design Principles

1. **No leaked types** — `crossbeam-channel`, `serde`, and `thiserror` types do not appear in public APIs. Internal types are `pub(crate)`.
2. **Stable IDs** — `ActorId` and `TimerId` are stable within a runtime instance and never reused.
3. **Explicit errors** — All fallible operations return typed errors (`RuntimeError`, `ActorError`, `ComputeError`).
4. **Ownership transfer** — Messages transfer ownership; no shared mutable state by default.
5. **Type safety** — Capabilities are generic over `ResourceHandle`, ensuring compile-time type safety.
