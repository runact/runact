# Chapter 18: API Reference

This chapter is a quick-reference guide to Runact's public API as of
v1.3.3. For detailed explanations, see the chapters in Parts I–V.

All signatures below are taken from the crate source. Items marked
**feature-gated** require the `hot_code_loading` cargo feature.

---

## 18.1 runact::Runtime

```rust
pub struct Runtime { /* private fields */ }

impl Runtime {
    pub fn new() -> Result<Self, RuntimeError>;
    pub fn with_config(config: RuntimeConfig) -> Result<Self, RuntimeError>;

    // Actors
    pub fn spawn<A: Actor>(&mut self, actor: A) -> Result<ActorId, RuntimeError>;
    pub fn spawn_supervisor(
        &self,
        strategy: RestartStrategy,
        children: Vec<ChildSpec>,
    ) -> Result<ActorId, RuntimeError>;
    pub fn list_actors(&self) -> Result<Vec<ActorInfo>, RuntimeError>;
    pub fn inspect_actor(&self, id: ActorId) -> Result<Option<ActorInfo>, RuntimeError>;

    // Messaging
    pub fn send<M: Send + 'static>(
        &self, target: ActorId, message: M,
    ) -> Result<(), RuntimeError>;
    pub fn send_blocking<M: Send + 'static>(
        &self, target: ActorId, message: M,
    ) -> Result<(), RuntimeError>;
    pub fn request<M: Send + 'static>(
        &self, target: ActorId, message: M,
    ) -> Result<RequestHandle, RuntimeError>;
    pub fn request_blocking<M: Send + 'static, R: Send + 'static>(
        &self, target: ActorId, message: M, timeout: Option<Duration>,
    ) -> Result<R, RuntimeError>;
    pub fn sender(&self) -> RuntimeSender;

    // Async tasks
    pub fn spawn_task<F, T>(&self, future: F) -> Result<TaskHandle<T>, TaskError>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static;
    pub fn sleep(duration: Duration) -> SleepFuture;
    pub fn timeout<F, T>(duration: Duration, future: F) -> TimeoutFuture<F>
    where
        F: Future<Output = T>;

    // Compute
    pub fn compute(&self) -> &ComputeScheduler;

    // Timers
    pub fn schedule_timer<M: Send + 'static>(
        &self, duration: Duration, target: ActorId, message: M,
    ) -> TimerId;
    pub fn schedule_interval<M: Clone + Send + Sync + 'static>(
        &self, interval: Duration, target: ActorId, message: M,
    ) -> TimerId;
    pub fn cancel_timer(&self, timer_id: TimerId);

    // Observability and lifecycle
    pub fn stats(&self) -> RuntimeStats;
    pub fn shutdown(&mut self) -> Result<(), RuntimeError>;

    // Hot code loading (feature-gated)
    #[cfg(feature = "hot_code_loading")]
    pub fn load_actor_module<P: AsRef<Path>>(
        &mut self, module_name: &str, library_path: P,
    ) -> Result<CodeChangeResult, RuntimeError>;
    #[cfg(feature = "hot_code_loading")]
    pub fn module_version(&self, module_name: &str) -> Option<String>;
    #[cfg(feature = "hot_code_loading")]
    pub fn module_versions(&self, module_name: &str) -> Vec<String>;
    #[cfg(feature = "hot_code_loading")]
    pub fn actor_factory(&self) -> &ActorFactory;
}
```

> **Note:** `Runtime::sleep` and `Runtime::timeout` are associated
> functions — call them as `Runtime::sleep(d)`, not `runtime.sleep(d)`.
> `spawn` requires `&mut self`; everything else in this list takes `&self`
> (except `shutdown` and `load_actor_module`, which take `&mut self`).

### RequestHandle

```rust
pub struct RequestHandle { /* private fields */ }

impl RequestHandle {
    pub fn id(&self) -> u64;
    pub fn recv(self) -> Result<Box<dyn Any + Send>, RuntimeError>;
    pub fn try_recv(&self) -> Result<Box<dyn Any + Send>, RuntimeError>;
    pub fn recv_timeout(&self, timeout: Duration)
        -> Result<Box<dyn Any + Send>, RuntimeError>;
}
```

The reply is boxed; downcast it to the expected type:

```rust
let reply = runtime.request(counter, CounterMsg::Get)?.recv()?;
let value = *reply.downcast::<u64>().unwrap();
```

`try_recv` returns `Err(RuntimeError::RuntimeStopped)` when no reply is
available (the implementation maps both empty and disconnected channels to
this error).

## 18.2 runact::RuntimeSender

```rust
pub struct RuntimeSender { /* private fields */ }

impl RuntimeSender {
    pub fn send<M: Send + 'static>(
        &self, target: ActorId, message: M,
    ) -> Result<(), RuntimeError>;
    pub fn request<M: Send + 'static>(
        &self, target: ActorId, message: M,
    ) -> Result<RequestHandle, RuntimeError>;
}
```

`RuntimeSender` is `Clone` and can be sent to any thread. It can message
and issue requests to existing actors, but it **cannot spawn** actors —
spawning requires `Runtime::spawn(&mut self)`.

## 18.3 Actor Types

```rust
pub trait Actor: Send + 'static {
    type Message: Send + 'static;
    fn handle(
        &mut self,
        message: Self::Message,
        context: &mut ActorContext,
    ) -> Result<(), ActorError>;
}

pub struct ActorContext { /* private fields */ }

impl ActorContext {
    pub fn actor_id(&self) -> ActorId;
    pub fn send_to<M: Send + 'static>(
        &self, target: ActorId, message: M,
    ) -> Result<(), RuntimeError>;
    pub fn reply<M: Send + 'static>(
        &self, message: M,
    ) -> Result<(), RuntimeError>;
    pub fn is_request(&self) -> bool;
    pub fn spawn_compute<F, T>(
        &self, job: F,
    ) -> Result<ComputeHandle<T>, ComputeError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static;
    pub fn schedule_timer<M: Send + 'static>(
        &self, duration: Duration, message: M,
    ) -> Result<TimerId, RuntimeError>;
    pub fn schedule_interval<M: Clone + Send + Sync + 'static>(
        &self, interval: Duration, message: M,
    ) -> Result<TimerId, RuntimeError>;
    pub fn cancel_timer(&self, timer_id: TimerId);

    // Hot code loading (feature-gated)
    #[cfg(feature = "hot_code_loading")]
    pub fn spawn_child(
        &self, module_name: &str, initial_state: Vec<u8>,
    ) -> Result<(), RuntimeError>;
}

pub struct ActorId(u64);
impl ActorId {
    pub fn new(id: u64) -> Self;
    pub fn as_u64(self) -> u64;
}

pub enum ActorError {
    Handler(String),
    Cancelled,
    Timeout,
    Panic(String),
}
```

`ActorError` has no `From<RuntimeError>`/`From<ComputeError>` impls, so
inside `handle` map errors explicitly:

```rust
ctx.reply(value)
    .map_err(|e| ActorError::Handler(e.to_string()))?;
```

## 18.4 Error Types

```rust
pub enum RuntimeError {
    ActorNotFound(ActorId),
    MailboxFull(ActorId),
    ActorExists(ActorId),
    ShutdownTimeout,
    RuntimeStopped,
}

pub enum TaskError {
    Panic(String),
    ExecutorShutdown,
    Timeout,
}

pub enum ComputeError {
    QueueFull,
    SchedulerShutdown,
    WorkerPanic(String),
}
```

`ComputeError::QueueFull` exists but is not currently produced: the
compute queue is unbounded, so `queue_capacity` acts as a documented soft
limit only.

## 18.5 Config Types

```rust
pub struct RuntimeConfig {
    pub compute: ComputeConfig,
    pub mailbox_capacity: usize,      // default: 1000
    pub shutdown_timeout: Duration,   // default: 5s
}

pub struct ComputeConfig {
    pub max_workers: usize,           // default: available_parallelism()
    pub queue_capacity: usize,        // default: 1024 (soft limit)
    pub task_timeout: Option<Duration>, // not yet enforced
}

pub struct RuntimeStats {
    pub actor_count: usize,
    pub request_count: u64,
}

pub struct ActorInfo {
    pub id: ActorId,
    pub name: Option<String>,
    pub mailbox_depth: usize,
}
```

## 18.6 runact-web Public API

```rust
// Crate root modules
pub mod extract;      // Path, Query, Json, Header
pub mod headers;      // Headers, HttpError
pub mod middleware;   // Next, Middleware
pub mod request;      // Method, Request
pub mod response;     // Response, StatusCode
pub mod router;       // Handler, IntoResponse, RouteMatch, Router
pub mod websocket;

// WebSocket submodules
pub mod websocket::frame;      // Frame, OpCode, FrameError
pub mod websocket::async_ws;   // AsyncWebSocket, AsyncWriter, WebSocketConfig,
                               // Message, SendError
pub mod websocket::server;     // WebSocketServer, ServerEvent, HandshakeError,
                               // ConnectionWriter
pub mod websocket::connection; // WebSocketConnection
pub mod websocket::runact_tcp; // RunactTcpStream
pub mod websocket::upgrade;    // UpgradeError, validate_upgrade_request,
                               // build_accept_key, build_upgrade_response
```

Key constructor notes:

- `WebSocketServer::bind(stream, "/path")` takes an already-accepted
  stream plus the expected path — it does not bind a listening socket.
- `ServerEvent::Error` carries a `String`, not a `HandshakeError`.
- `Response` has no `Default` impl; construct all five fields
  (`status`, `reason`, `version`, `headers`, `body`) or use
  `IntoResponse` for `&str`, `StatusCode`, and `(StatusCode, Vec<u8>)`.

## 18.7 Task Types

```rust
pub struct TaskHandle<T> {
    pub fn id(&self) -> TaskId;
    pub fn recv(&self) -> Result<T, TaskError> where T: Clone + 'static;
    pub fn try_recv(&self) -> Option<Result<T, TaskError>>
        where T: Clone + 'static;
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, TaskError>
        where T: Clone + 'static;
}

pub struct ComputeHandle<T> {
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
    pub fn try_recv(&self) -> Option<Result<T, ComputeError>> where T: 'static;
    pub fn recv(&self) -> Result<T, ComputeError> where T: 'static;
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, ComputeError>
        where T: 'static;
}

pub struct CancellationToken { /* private fields */ }
impl CancellationToken {
    pub fn new() -> Self;
    pub fn is_cancelled(&self) -> bool;
    pub fn cancel(&self);
    pub fn child_token(&self) -> CancellationToken;
    pub fn cancelled(&self) -> CancelledFuture;
}

pub struct TaskGroup { /* private fields */ }
impl TaskGroup {
    pub fn new() -> Self;
    pub fn spawn<F>(&self, runtime: &Runtime, future: F) -> TaskHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;
    pub fn cancel(&self);
    pub fn cancellation_token(&self) -> CancellationToken;
    pub fn is_cancelled(&self) -> bool;
}
```

`ComputeHandle::recv` and `recv_timeout` take `&self`, so a stored handle
can be polled repeatedly. `TaskHandle` caches its outcome, which is why
its methods require `T: Clone`.

> **TaskGroup semantics:** `TaskGroup::spawn` takes the runtime explicitly
> and returns the task's `TaskHandle`. Dropping the group does **not**
> cancel its children today; call `group.cancel()` explicitly.
