# Runact Runtime

## Overview

The Runact runtime is the execution kernel. It manages processes, message delivery, scheduling, supervision, and timers. Uses a custom BEAM-style scheduler with work stealing. No Tokio types leak through public APIs.

## Core Concepts

### Process

A Process is an independent unit of computation with:

- Stable `ProcessId`
- Owns mutable state
- Receives messages sequentially
- Processes one message at a time
- Can send messages to other processes
- Can spawn child processes
- Can be supervised
- Can be monitored
- Can be cancelled
- Can schedule timers

```rust
pub trait Process: Send + 'static {
    type Message: Send + 'static;

    fn handle(
        &mut self,
        message: Self::Message,
        context: &mut ProcessContext,
    ) -> Result<(), ProcessError>;
}
```

### ProcessId

A stable, unique identifier for a process.

- Created at spawn time
- Never reused within a runtime instance
- Serializable
- Orderable (for debugging)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProcessId(u64);
```

### ProcessContext

Provided to process handlers. Provides access to:

- Sending messages
- Spawning child processes
- Scheduling timers
- Accessing runtime information

```rust
pub struct ProcessContext {
    process_id: ProcessId,
    sender: MessageSender,
    runtime_handle: RuntimeHandle,
}
```

### ProcessLifecycle

```
spawned → running → stopping → stopped
    │                  │
    │                  └──→ terminated
    │
    └──→ failed → restarting → running
```

States:

- **spawned** — Process created, not yet scheduled
- **running** — Process is executing
- **stopping** — Graceful shutdown initiated
- **stopped** — Process completed normally
- **terminated** — Process was forcibly stopped
- **failed** — Process crashed or returned error
- **restarting** — Supervisor is restarting the process

## Mailbox

A typed message queue for a process.

### Requirements

- FIFO ordering within a process
- Configurable capacity
- Backpressure policy
- Cancellation support
- Shutdown behavior
- Metrics

### Backpressure Strategies

When the mailbox is full:

| Strategy | Behavior |
|----------|----------|
| `Reject` | Return error to sender |
| `Block` | Sender waits until space available |
| `DropLowPriority` | Drop lowest priority message |
| `Coalesce` | Merge compatible messages |
| `Escalate` | Notify supervisor |

The correct strategy depends on message type.

### Mailbox Interface

```rust
pub struct Mailbox<T> {
    capacity: usize,
    backpressure: BackpressurePolicy,
    metrics: MailboxMetrics,
}

impl<T> Mailbox<T> {
    pub fn send(&self, message: T) -> Result<(), MailboxError>;
    pub fn receive(&self) -> Option<T>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

### Metrics

```rust
pub struct MailboxMetrics {
    pub depth: usize,
    pub high_watermark: usize,
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub messages_rejected: u64,
}
```

## Scheduler

BEAM-style scheduler with work stealing.

### Architecture

```
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
│  Process A  │  Process D  │  Process G  │  Process J  │
│  (N reduc)  │  (N reduc)  │  (N reduc)  │  (N reduc)  │
└─────────────┴─────────────┴─────────────┴──────────────┘
                    │
            Work Stealing
```

### Requirements

- Lightweight process creation
- Work stealing across cores
- Per-worker run queues
- Cooperative scheduling with reduction counting
- Cancellation
- Timers
- Process lifecycle management

### Scheduler Interface

```rust
pub struct Scheduler {
    workers: Vec<Worker>,
}

impl Scheduler {
    pub fn new(num_workers: Option<usize>) -> Self;
    pub fn start(&mut self);
    pub fn stop(&mut self);
    pub fn worker_count(&self) -> usize;
}
```

### Worker

Per-core scheduler with own run queue.

```rust
pub struct Worker {
    id: usize,
    run_queue: Arc<RunQueue>,
    reduction_counter: ReductionCounter,
}
```

### RunQueue

Lock-free MPSC queue per worker.

```rust
pub struct RunQueue {
    // Lock-free implementation
}

impl RunQueue {
    pub fn new() -> Self;
    pub fn push(&self, task: Arc<dyn Send + Sync + 'static>);
    pub fn pop(&self) -> Option<Arc<dyn Send + Sync + 'static>>;
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
}
```

### ReductionCounter

Prevents starvation by counting message processing.

```rust
pub const MAX_REDUCTIONS: u64 = 4000;

pub struct ReductionCounter {
    count: AtomicU64,
}

impl ReductionCounter {
    pub fn new() -> Self;
    pub fn increment(&self) -> u64;
    pub fn reset(&self);
    pub fn should_preempt(&self) -> bool;
}
```

### Exit Criterion

The scheduler must support 100K+ concurrent processes reliably.

## Compute Scheduler

For CPU-intensive tasks that would starve actors.

### Architecture

```
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
                    │
            Work Stealing
```

### Requirements

- CPU-intensive tasks run to completion
- Work stealing across cores
- Bounded queue for backpressure
- Panic isolation at worker boundary
- Oneshot result channel
- Never block actors

### Compute Scheduler Interface

```rust
pub struct ComputeScheduler {
    pool: ThreadPool,
    sender: crossbeam_channel::Sender<Task>,
    config: ComputeConfig,
}

pub struct ComputeConfig {
    pub max_workers: usize,
    pub queue_capacity: usize,
}

impl ComputeScheduler {
    pub fn new(config: ComputeConfig) -> Result<Self, ComputeError>;
    pub fn spawn<F, T>(&self, job: F) -> Result<ComputeHandle<T>, ComputeError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static;
    pub fn shutdown(self);
}
```

### Task

```rust
pub struct Task {
    pub id: TaskId,
    pub job: Box<dyn FnOnce() -> Box<dyn Any + Send> + Send>,
    pub result_sender: oneshot::Sender<ComputeResult>,
    pub created_at: Instant,
}
```

### ComputeResult

```rust
pub enum ComputeResult {
    Ok(Box<dyn Any + Send>),
    Err(ComputeError),
    Panic(String),
    Rejected,
}
```

### ComputeHandle

```rust
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

### Panic Handling

Workers catch panics at the boundary:

```rust
// Inside worker thread
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    (task.job)()
}));

match result {
    Ok(value) => task.result_sender.send(ComputeResult::Ok(Box::new(value))),
    Err(panic) => {
        let msg = panic.downcast_ref::<&str>()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown panic".to_string());
        task.result_sender.send(ComputeResult::Panic(msg))
    }
}
```

### Backpressure

When queue is full, reject immediately:

```rust
match self.sender.try_send(task) {
    Ok(()) => Ok(ComputeHandle::new(result_receiver)),
    Err(crossbeam_channel::TrySendError::Full(_)) => {
        Err(ComputeError::QueueFull)
    }
    Err(crossbeam_channel::TrySendError::Closed(_)) => {
        Err(ComputeError::SchedulerShutdown)
    }
}
```

### Exit Criterion

- CPU-intensive tasks run to completion
- Actors never block on compute tasks
- Panic isolation works
- Backpressure prevents memory exhaustion

## Timer

Scheduled message delivery.

### Types

- **One-shot** — Fire once after delay
- **Periodic** — Fire repeatedly at interval
- **Deadline** — Fire at specific time

### Timer Interface

```rust
pub struct Timer {
    id: TimerId,
    process_id: ProcessId,
    scheduled_at: Instant,
    message: Box<dyn Any + Send>,
}

impl Timer {
    pub fn after(duration: Duration, message: impl Into<Box<dyn Any + Send>>) -> Self;
    pub fn at(time: Instant, message: impl Into<Box<dyn Any + Send>>) -> Self;
    pub fn every(interval: Duration, message: impl Into<Box<dyn Any + Send>>) -> Self;
}
```

## Cancellation

Cooperative cancellation via tokens.

### CancellationToken

```rust
pub struct CancellationToken {
    inner: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self;
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
}
```

### Usage

```rust
fn my_process(context: &mut ProcessContext) {
    loop {
        if context.cancellation_token().is_cancelled() {
            break;
        }
        if let Some(msg) = context.receive() {
            handle_message(msg);
        }
    }
}
```

## Runtime

Top-level coordinator that owns all processes.

### Runtime Interface

```rust
pub struct Runtime {
    scheduler: Scheduler,
    processes: HashMap<ProcessId, ProcessHandle>,
    supervisors: HashMap<ProcessId, SupervisorHandle>,
}

impl Runtime {
    pub fn new() -> Self;
    pub fn spawn<P: Process>(&self, process: P) -> ProcessRef;
    pub fn spawn_supervisor(&self, config: SupervisorConfig) -> ProcessRef;
    pub fn list_processes(&self) -> Vec<ProcessInfo>;
    pub fn inspect_process(&self, id: ProcessId) -> Option<ProcessInfo>;
    pub fn shutdown(&self) -> ShutdownHandle;
}
```

### Shutdown

Graceful shutdown:

1. Stop accepting new messages
2. Wait for in-flight messages to complete
3. Notify processes of shutdown
4. Wait for processes to stop (with timeout)
5. Force-kill remaining processes

```rust
pub struct ShutdownConfig {
    pub timeout: Duration,
    pub force_after: bool,
}

impl Runtime {
    pub fn shutdown(&self, config: ShutdownConfig);
}
```

## Supervisor

Manages child process lifecycle and restart policies.

### Restart Strategies

**one-for-one** (initial implementation):

```
Child crashes → Supervisor restarts that child
```

**one-for-all** (later):

```
Child crashes → All children restart
```

**rest-for-one** (later):

```
Child crashes → That child and all children started after it restart
```

### Supervisor Interface

```rust
pub struct Supervisor {
    id: ProcessId,
    strategy: RestartStrategy,
    children: Vec<ChildSpec>,
    restart_count: HashMap<ProcessId, usize>,
    backoff: BackoffConfig,
}

impl Supervisor {
    pub fn new(strategy: RestartStrategy) -> Self;
    pub fn child(&mut self, spec: ChildSpec) -> &mut Self;
    pub fn start(&mut self) -> Result<(), SupervisorError>;
    pub fn handle_child_failure(&mut self, child_id: ProcessId) -> Result<(), SupervisorError>;
}
```

### Child Specification

```rust
pub struct ChildSpec {
    pub name: String,
    pub process_type: ProcessType,
    pub restart_policy: RestartPolicy,
    pub shutdown_timeout: Duration,
}

pub enum RestartPolicy {
    Permanent,  // Always restart
    Temporary,  // Never restart
    Transient,  // Restart only on abnormal exit
}
```

### Backoff

Prevent infinite restart loops.

```rust
pub struct BackoffConfig {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub max_restarts: usize,
    pub within: Duration,  // Reset counter after this period
}
```

### Crash Reporting

When a child crashes, the supervisor:

1. Logs the crash with full context
2. Updates restart counters
3. Applies backoff if needed
4. Restarts the child
5. Escalates if restart limit exceeded

## Process Monitoring

Separate from supervision. Monitoring is observation, not ownership.

### Monitor

```rust
pub struct Monitor {
    watcher: ProcessId,
    watched: ProcessId,
    notification: oneshot::Sender<ProcessEvent>,
}

pub enum ProcessEvent {
    Stopped(ProcessId),
    Failed(ProcessId, ProcessError),
}
```

### Usage

```rust
let monitor = runtime.monitor(watcher_id, watched_id);
match monitor.await {
    Ok(event) => handle_event(event),
    Err(_) => timeout,
}
```

## Observability

### Structured Tracing

Every important operation should include:

```rust
pub struct SpanContext {
    pub timestamp: Instant,
    pub process_id: ProcessId,
    pub parent_process_id: Option<ProcessId>,
    pub correlation_id: Option<CorrelationId>,
    pub operation: String,
    pub duration: Option<Duration>,
    pub result: Option<String>,
    pub error: Option<String>,
}
```

### Metrics

```rust
pub struct RuntimeMetrics {
    pub process_count: usize,
    pub mailbox_depth: HashMap<ProcessId, usize>,
    pub message_rate: f64,
    pub message_latency: Duration,
    pub restart_count: usize,
}
```

## Exit Criterion

Phase 1 is complete when:

- 100K+ test processes communicate reliably
- Process creation is lightweight
- Message delivery is reliable
- Work stealing across cores
- No starvation (reduction counting works)
- Timers fire correctly
- Cancellation works
- Shutdown is graceful
- Metrics are available
