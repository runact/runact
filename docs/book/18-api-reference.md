# Chapter 18: API Reference

This chapter is a quick-reference guide to Runact's public API. For
detailed explanations, see the chapters in Parts I–V.

---

## 18.1 runact::Runtime

```rust
pub struct Runtime {
    pub fn new() -> Result<Self, RuntimeError>;
    pub fn with_config(config: RuntimeConfig) -> Result<Self, RuntimeError>;

    pub fn spawn<A: Actor>(&mut self, actor: A) -> Result<ActorId, RuntimeError>;
    pub fn spawn_supervisor(
        &self, strategy: RestartStrategy, children: Vec<ChildSpec>
    ) -> Result<ActorId, RuntimeError>;

    pub fn send<M: Send + 'static>(
        &self, target: ActorId, message: M
    ) -> Result<(), RuntimeError>;
    pub fn send_blocking<M: Send + 'static>(
        &self, target: ActorId, message: M
    ) -> Result<(), RuntimeError>;

    pub fn request<M: Send + 'static>(
        &self, target: ActorId, message: M
    ) -> Result<RequestHandle, RuntimeError>;

    pub fn spawn_task<F, T>(
        &self, future: F
    ) -> Result<TaskHandle<T>, TaskError>
    where F: Future<Output = T> + Send + 'static, T: Send + 'static;

    pub fn schedule_timer<M: Send + 'static>(
        &self, duration: Duration, target: ActorId, message: M
    ) -> TimerId;
    pub fn cancel_timer(&self, timer_id: TimerId);

    pub fn sender(&self) -> RuntimeSender;
    pub fn shutdown(&mut self) -> Result<(), RuntimeError>;

    pub fn list_actors(&self) -> Result<Vec<ActorInfo>, RuntimeError>;
    pub fn inspect_actor(&self, id: ActorId) -> Result<Option<ActorInfo>, RuntimeError>;

    pub fn compute(&self) -> &ComputeScheduler;
}
```

## 18.2 runact::RuntimeSender

```rust
pub struct RuntimeSender {
    pub fn send<M: Send + 'static>(
        &self, target: ActorId, message: M
    ) -> Result<(), RuntimeError>;
    pub fn spawn<A: Actor>(&self, actor: A) -> Result<ActorId, RuntimeError>;
}
```

`RuntimeSender` is `Clone`. It can be sent to any thread.

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

pub struct ActorContext {
    pub fn actor_id(&self) -> ActorId;
    pub fn send_to<M: Send + 'static>(
        &self, target: ActorId, message: M
    ) -> Result<(), RuntimeError>;
    pub fn reply<M: Send + 'static>(
        &self, message: M
    ) -> Result<(), RuntimeError>;
    pub fn is_request(&self) -> bool;
    pub fn spawn_compute<F, T>(
        &self, job: F
    ) -> Result<ComputeHandle<T>, ComputeError>
    where F: FnOnce() -> T + Send + 'static, T: Send + 'static;
    pub fn schedule_timer<M: Send + 'static>(
        &self, duration: Duration, message: M
    ) -> Result<TimerId, RuntimeError>;
    pub fn schedule_interval<M: Clone + Send + Sync + 'static>(
        &self, interval: Duration, message: M
    ) -> Result<TimerId, RuntimeError>;
    pub fn cancel_timer(&self, timer_id: TimerId);
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

## 18.4 Error Types

```rust
pub enum RuntimeError {
    ActorNotFound(ActorId),
    MailboxFull(ActorId),
    RuntimeStopped,
    InvalidConfig(String),
}

pub enum TaskError {
    Panic(String),
    ExecutorShutdown,
    Timeout,
}

pub enum ComputeError {
    SchedulerShutdown,
    QueueFull,
    WorkerPanic(String),
}
```

## 18.5 Config Types

```rust
pub struct RuntimeConfig {
    pub compute: ComputeConfig,
    pub mailbox_capacity: usize,
    pub shutdown_timeout: Duration,
}

pub struct ComputeConfig {
    pub max_workers: usize,
    pub queue_capacity: usize,
    pub task_timeout: Option<Duration>,
}
```

## 18.6 runact-web Public API

```rust
// HTTP
pub mod request::{Method, Request};
pub mod response::{Response, StatusCode};
pub mod headers::{Headers};
pub mod router::{Router, IntoResponse};

// WebSocket
pub mod websocket::{
    frame::{Frame, OpCode},
    async_ws::{AsyncWebSocket, AsyncWriter, WebSocketConfig, Message, SendError},
    server::{WebSocketServer, ServerEvent, HandshakeError, ConnectionWriter},
    upgrade::{validate_upgrade_request, build_upgrade_response},
};
```

## 18.7 Task Types

```rust
pub struct TaskHandle<T> {
    pub fn id(&self) -> TaskId;
    pub fn recv(&self) -> Result<T, TaskError> where T: Clone + 'static;
    pub fn try_recv(&self) -> Option<Result<T, TaskError>> where T: Clone + 'static;
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, TaskError> where T: Clone + 'static;
}

pub struct ComputeHandle<T> {
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
    pub fn try_recv(&self) -> Option<Result<T, ComputeError>> where T: 'static;
    pub fn recv(&self) -> Result<T, ComputeError> where T: 'static;
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, ComputeError> where T: 'static;
}

pub struct CancellationToken {
    pub fn is_cancelled(&self) -> bool;
    pub fn cancel(&self);
    pub fn child_token(&self) -> CancellationToken;
}
```
