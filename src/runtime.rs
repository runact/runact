#[cfg(feature = "hot_code_loading")]
use crate::actor::SpawnRequest;
use crate::actor::{Actor, ActorContext, ActorId};
use crate::compute::{ComputeConfig, ComputeScheduler};
use crate::error::RuntimeError;
#[cfg(feature = "hot_code_loading")]
use crate::hotswap::{ActorFactory, CodeChangeResult, CodeManager, SpawnListener};
use crate::scheduler::{MAX_REDUCTIONS, ReductionCounter, Scheduler};
use crate::supervision::{ChildSpec, RestartStrategy, Supervisor};
use crate::task::{Executor, TaskError, TaskHandle};
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
#[cfg(feature = "hot_code_loading")]
use std::path::Path;
use std::pin::Pin;
#[cfg(feature = "hot_code_loading")]
use std::sync::Mutex as StdMutex;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use crate::timer::{TimerId, TimerService};

// ============================================================================
// Message envelope and request handling
// ============================================================================

/// Envelope wrapping messages sent to actors.
pub(crate) enum MessageEnvelope {
    Message(Box<dyn Any + Send>),
    Request {
        _request_id: u64,
        payload: Box<dyn Any + Send>,
        reply_sender: crossbeam_channel::Sender<Box<dyn Any + Send>>,
    },
}

/// Handle to a pending request. Receives the reply asynchronously.
#[must_use]
#[derive(Debug)]
pub struct RequestHandle {
    id: u64,
    receiver: crossbeam_channel::Receiver<Box<dyn Any + Send>>,
}

impl RequestHandle {
    /// Get the request's correlation ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Block until a reply is received.
    pub fn recv(self) -> Result<Box<dyn Any + Send>, RuntimeError> {
        self.receiver
            .recv()
            .map_err(|_| RuntimeError::RuntimeStopped)
    }

    /// Try to receive a reply without blocking.
    pub fn try_recv(&self) -> Result<Box<dyn Any + Send>, RuntimeError> {
        self.receiver
            .try_recv()
            .map_err(|_| RuntimeError::RuntimeStopped)
    }

    /// Receive a reply with a timeout.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Box<dyn Any + Send>, RuntimeError> {
        self.receiver
            .recv_timeout(timeout)
            .map_err(|_| RuntimeError::RuntimeStopped)
    }
}

/// Information about a spawned actor.
#[must_use]
#[derive(Debug, Clone)]
pub struct ActorInfo {
    /// Unique actor identifier.
    pub id: ActorId,
    /// Optional human-readable name (e.g., "supervisor").
    pub name: Option<String>,
    /// Current number of messages queued in the actor's mailbox.
    pub mailbox_depth: usize,
}

// ============================================================================
// Runtime
// ============================================================================

/// Top-level runtime coordinator.
#[must_use]
pub struct Runtime {
    scheduler: Scheduler,
    compute: ComputeScheduler,
    timers: TimerService,
    tasks: Executor,
    actors: Arc<RwLock<HashMap<ActorId, ActorInfo>>>,
    senders: Arc<RwLock<HashMap<ActorId, crossbeam_channel::Sender<MessageEnvelope>>>>,
    request_counter: AtomicU64,
    stop: Arc<AtomicBool>,
    mailbox_capacity: usize,
    shutdown_timeout: Duration,
    /// Number of actor loops still running.
    pending_actors: Arc<AtomicUsize>,
    /// Hot code loading: tracks versions and vtables for dynamic actors.
    #[cfg(feature = "hot_code_loading")]
    code_manager: Arc<StdMutex<CodeManager>>,
    /// Factory for spawning versioned actors.
    #[cfg(feature = "hot_code_loading")]
    factory: Option<ActorFactory>,
    /// Channel for actor-initiated child spawns (Pattern 2).
    #[cfg(feature = "hot_code_loading")]
    spawn_request_tx: Option<crossbeam_channel::Sender<SpawnRequest>>,
}

impl Runtime {
    /// Create a new runtime.
    pub fn new() -> Result<Self, RuntimeError> {
        Self::with_config(RuntimeConfig::default())
    }

    /// Create a new runtime with configuration.
    pub fn with_config(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let scheduler = Scheduler::new();
        let compute =
            ComputeScheduler::new(config.compute).map_err(|_| RuntimeError::RuntimeStopped)?;

        let senders = Arc::new(RwLock::new(HashMap::new()));
        let timers = TimerService::new(senders.clone());
        let tasks = Executor::new();

        #[cfg(feature = "hot_code_loading")]
        let code_manager = Arc::new(StdMutex::new(CodeManager::new()));

        #[cfg(feature = "hot_code_loading")]
        let (mut spawn_listener, spawn_request_tx) = SpawnListener::new(
            code_manager.clone(),
            senders.clone(),
            config.mailbox_capacity,
        );

        let stop = Arc::new(AtomicBool::new(false));
        let pending_actors = Arc::new(AtomicUsize::new(0));

        #[cfg(feature = "hot_code_loading")]
        {
            spawn_listener.set_stop(stop.clone());
            spawn_listener.set_pending(pending_actors.clone());
        }

        let runtime = Self {
            scheduler,
            compute,
            timers,
            tasks,
            actors: Arc::new(RwLock::new(HashMap::new())),
            senders: senders.clone(),
            request_counter: AtomicU64::new(0),
            stop: stop.clone(),
            mailbox_capacity: config.mailbox_capacity,
            shutdown_timeout: config.shutdown_timeout,
            pending_actors: pending_actors.clone(),
            #[cfg(feature = "hot_code_loading")]
            code_manager: code_manager.clone(),
            #[cfg(feature = "hot_code_loading")]
            factory: Some(ActorFactory::new(
                code_manager,
                RuntimeSender {
                    senders: senders.clone(),
                    request_counter: Arc::new(AtomicU64::new(0)),
                },
            )),
            #[cfg(feature = "hot_code_loading")]
            spawn_request_tx: Some(spawn_request_tx),
        };

        #[cfg(feature = "hot_code_loading")]
        {
            spawn_listener.set_actors(runtime.actors.clone());
            spawn_listener.spawn_listener_thread();
        }

        Ok(runtime)
    }

    /// Spawn an actor.
    pub fn spawn<A: Actor>(&mut self, mut actor: A) -> Result<ActorId, RuntimeError> {
        let id = ActorId::new(
            self.actors
                .read()
                .map_err(|_| RuntimeError::RuntimeStopped)?
                .len() as u64
                + 1,
        );

        let info = ActorInfo {
            id,
            name: None,
            mailbox_depth: 0,
        };

        self.actors
            .write()
            .map_err(|_| RuntimeError::RuntimeStopped)?
            .insert(id, info);

        let (tx, rx) = crossbeam_channel::bounded::<MessageEnvelope>(self.mailbox_capacity);

        self.senders
            .write()
            .map_err(|_| RuntimeError::RuntimeStopped)?
            .insert(id, tx.clone());

        let mut ctx = ActorContext::new(id, tx);
        ctx.set_senders(self.senders.clone());
        ctx.set_timer_handle(self.timers.handle());
        #[cfg(feature = "hot_code_loading")]
        {
            if let Some(ref spawn_tx) = self.spawn_request_tx {
                ctx.set_spawn_child_req_tx(spawn_tx.clone());
            }
        }
        if let Some(compute_tx) = self.compute.sender() {
            ctx.set_compute_sender(compute_tx);
        }
        let stop = self.stop.clone();
        let reductions = ReductionCounter::new();
        let pending_actors = self.pending_actors.clone();
        pending_actors.fetch_add(1, Ordering::SeqCst);

        tracing::info!(actor_id = %id, "spawned actor");

        self.scheduler.spawn(move || {
            let _span = tracing::info_span!("actor_loop", actor_id = %id).entered();
            loop {
                if stop.load(Ordering::Relaxed) {
                    match rx.try_recv() {
                        Ok(envelope) => {
                            match envelope {
                                MessageEnvelope::Message(msg) => {
                                    if let Ok(msg) = msg.downcast::<A::Message>() {
                                        ctx.set_reply_sender(None);
                                        let _ = actor.handle(*msg, &mut ctx);
                                    }
                                }
                                MessageEnvelope::Request {
                                    payload,
                                    reply_sender,
                                    ..
                                } => {
                                    if let Ok(msg) = payload.downcast::<A::Message>() {
                                        ctx.set_reply_sender(Some(reply_sender));
                                        let _ = actor.handle(*msg, &mut ctx);
                                    }
                                }
                            }
                            continue;
                        }
                        Err(_) => {
                            tracing::info!(actor_id = %id, "actor stopped");
                            pending_actors.fetch_sub(1, Ordering::SeqCst);
                            break;
                        }
                    }
                }
                match rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(envelope) => {
                        match envelope {
                            MessageEnvelope::Message(msg) => {
                                if let Ok(msg) = msg.downcast::<A::Message>() {
                                    tracing::trace!(actor_id = %id, "handling message");
                                    ctx.set_reply_sender(None);
                                    let _ = actor.handle(*msg, &mut ctx);
                                }
                            }
                            MessageEnvelope::Request {
                                payload,
                                reply_sender,
                                ..
                            } => {
                                if let Ok(msg) = payload.downcast::<A::Message>() {
                                    tracing::trace!(actor_id = %id, "handling request");
                                    ctx.set_reply_sender(Some(reply_sender));
                                    let _ = actor.handle(*msg, &mut ctx);
                                }
                            }
                        }
                        if reductions.increment() >= MAX_REDUCTIONS {
                            reductions.reset();
                            std::thread::yield_now();
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        reductions.reset();
                        continue;
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        pending_actors.fetch_sub(1, Ordering::SeqCst);
                        break;
                    }
                }
            }
        });

        Ok(id)
    }

    /// Send a fire-and-forget message to an actor (non-blocking).
    pub fn send<M: Send + 'static>(&self, target: ActorId, message: M) -> Result<(), RuntimeError> {
        tracing::debug!(target_id = %target, "send");
        let senders = self
            .senders
            .read()
            .map_err(|_| RuntimeError::RuntimeStopped)?;
        let sender = senders
            .get(&target)
            .ok_or(RuntimeError::ActorNotFound(target))?;
        sender
            .try_send(MessageEnvelope::Message(Box::new(message)))
            .map_err(|e| match e {
                crossbeam_channel::TrySendError::Full(_) => RuntimeError::MailboxFull(target),
                crossbeam_channel::TrySendError::Disconnected(_) => RuntimeError::RuntimeStopped,
            })
    }

    /// Send a fire-and-forget message to an actor, blocking until the mailbox accepts it.
    pub fn send_blocking<M: Send + 'static>(
        &self,
        target: ActorId,
        message: M,
    ) -> Result<(), RuntimeError> {
        tracing::debug!(target_id = %target, "send_blocking");
        let senders = self
            .senders
            .read()
            .map_err(|_| RuntimeError::RuntimeStopped)?;
        let sender = senders
            .get(&target)
            .ok_or(RuntimeError::ActorNotFound(target))?;
        sender
            .send(MessageEnvelope::Message(Box::new(message)))
            .map_err(|_| RuntimeError::RuntimeStopped)
    }

    /// Send a request to an actor and get a handle to receive the reply.
    pub fn request<M: Send + 'static>(
        &self,
        target: ActorId,
        message: M,
    ) -> Result<RequestHandle, RuntimeError> {
        let request_id = self.request_counter.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(target_id = %target, request_id, "request");
        let (reply_tx, reply_rx) = crossbeam_channel::bounded(1);

        let senders = self
            .senders
            .read()
            .map_err(|_| RuntimeError::RuntimeStopped)?;
        let sender = senders
            .get(&target)
            .ok_or(RuntimeError::ActorNotFound(target))?;

        sender
            .try_send(MessageEnvelope::Request {
                _request_id: request_id,
                payload: Box::new(message),
                reply_sender: reply_tx,
            })
            .map_err(|e| match e {
                crossbeam_channel::TrySendError::Full(_) => RuntimeError::MailboxFull(target),
                crossbeam_channel::TrySendError::Disconnected(_) => RuntimeError::RuntimeStopped,
            })?;

        Ok(RequestHandle {
            id: request_id,
            receiver: reply_rx,
        })
    }

    /// Send a request and block until the reply is received.
    pub fn request_blocking<M: Send + 'static, R: Send + 'static>(
        &self,
        target: ActorId,
        message: M,
        timeout: Option<Duration>,
    ) -> Result<R, RuntimeError> {
        let handle = self.request(target, message)?;
        match timeout {
            Some(dur) => {
                let reply = handle.recv_timeout(dur)?;
                reply
                    .downcast::<R>()
                    .map(|b| *b)
                    .map_err(|_| RuntimeError::RuntimeStopped)
            }
            None => {
                let reply = handle.recv()?;
                reply
                    .downcast::<R>()
                    .map(|b| *b)
                    .map_err(|_| RuntimeError::RuntimeStopped)
            }
        }
    }

    /// Spawn a supervisor that manages child actor lifecycles.
    pub fn spawn_supervisor(
        &self,
        strategy: RestartStrategy,
        children: Vec<ChildSpec>,
    ) -> Result<ActorId, RuntimeError> {
        let id = ActorId::new(
            self.actors
                .read()
                .map_err(|_| RuntimeError::RuntimeStopped)?
                .len() as u64
                + 1,
        );

        let mut supervisor = Supervisor::new(id, strategy);
        for child in children {
            supervisor.child(child);
        }

        let info = ActorInfo {
            id,
            name: Some("supervisor".to_string()),
            mailbox_depth: 0,
        };

        self.actors
            .write()
            .map_err(|_| RuntimeError::RuntimeStopped)?
            .insert(id, info);

        Ok(id)
    }

    /// Spawn an asynchronous task on the native async executor.
    ///
    /// Dropping the handle detaches the task: it still runs to completion.
    pub fn spawn_task<F, T>(&self, future: F) -> Result<TaskHandle<T>, TaskError>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.tasks.spawn(future)
    }

    /// Sleep for the specified duration within an async task.
    pub fn sleep(duration: Duration) -> SleepFuture {
        SleepFuture::new(duration)
    }

    /// Execute a future with a timeout.
    pub fn timeout<F, T>(duration: Duration, future: F) -> TimeoutFuture<F>
    where
        F: Future<Output = T>,
    {
        TimeoutFuture::new(duration, future)
    }

    /// List all actors.
    pub fn list_actors(&self) -> Result<Vec<ActorInfo>, RuntimeError> {
        let actors = self
            .actors
            .read()
            .map_err(|_| RuntimeError::RuntimeStopped)?;
        Ok(actors.values().cloned().collect())
    }

    /// Inspect an actor.
    pub fn inspect_actor(&self, id: ActorId) -> Result<Option<ActorInfo>, RuntimeError> {
        let actors = self
            .actors
            .read()
            .map_err(|_| RuntimeError::RuntimeStopped)?;
        Ok(actors.get(&id).cloned())
    }

    /// Get a reference to the compute scheduler.
    pub fn compute(&self) -> &ComputeScheduler {
        &self.compute
    }

    /// Schedule a one-shot timer.
    pub fn schedule_timer<M: Send + 'static>(
        &self,
        duration: Duration,
        target: ActorId,
        message: M,
    ) -> TimerId {
        self.timers.schedule(duration, target, Box::new(message))
    }

    /// Schedule a periodic timer.
    pub fn schedule_interval<M: Clone + Send + Sync + 'static>(
        &self,
        interval: Duration,
        target: ActorId,
        message: M,
    ) -> TimerId {
        let message = Arc::new(message);
        self.timers.schedule_interval(
            interval,
            target,
            Box::new(move || Box::new((*message).clone()) as Box<dyn Any + Send>),
        )
    }

    /// Get runtime statistics.
    pub fn stats(&self) -> RuntimeStats {
        let actors = self.actors.read().unwrap_or_else(|e| e.into_inner());
        RuntimeStats {
            actor_count: actors.len(),
            request_count: self.request_counter.load(Ordering::Relaxed),
        }
    }

    /// Get a clonable handle for sending messages to actors from external threads.
    pub fn sender(&self) -> RuntimeSender {
        RuntimeSender {
            senders: self.senders.clone(),
            request_counter: Arc::new(AtomicU64::new(self.request_counter.load(Ordering::Relaxed))),
        }
    }

    /// Cancel a scheduled timer.
    pub fn cancel_timer(&self, timer_id: TimerId) {
        self.timers.cancel(timer_id);
    }

    /// Gracefully shut down the runtime.
    pub fn shutdown(&mut self) -> Result<(), RuntimeError> {
        self.stop.store(true, Ordering::SeqCst);

        self.tasks.shutdown();

        let deadline = Instant::now() + self.shutdown_timeout;
        while self.pending_actors.load(Ordering::Relaxed) > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        tracing::info!(
            timeout_ms = self.shutdown_timeout.as_millis() as u64,
            "runtime shutdown"
        );

        self.senders
            .write()
            .map_err(|_| RuntimeError::RuntimeStopped)?
            .clear();

        self.actors
            .write()
            .map_err(|_| RuntimeError::RuntimeStopped)?
            .clear();

        self.timers.shutdown();
        self.scheduler.shutdown();
        self.compute.shutdown();
        Ok(())
    }

    // ========================================================================
    // Hot Code Loading (feature-gated)
    // ========================================================================

    #[cfg(feature = "hot_code_loading")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hot_code_loading")))]
    /// Load a new version of an actor module from a dynamic library.
    ///
    /// After loading, **new processes spawned for this module will use the
    /// new code version**. Existing processes continue running the old code
    /// until restarted or upgraded.
    pub fn load_actor_module<P: AsRef<Path>>(
        &mut self,
        module_name: &str,
        library_path: P,
    ) -> Result<CodeChangeResult, RuntimeError> {
        let result = {
            let mut cm = self.code_manager.lock().unwrap();
            let old_version = cm.load_new_version(module_name, library_path)?;
            let new_version = cm
                .current_version(module_name)
                .unwrap_or_else(|| "unknown".to_string());

            CodeChangeResult::Upgraded {
                module: module_name.to_string(),
                new_version,
                old_version,
            }
        };

        tracing::info!(?result, "hot code load complete");
        Ok(result)
    }

    #[cfg(feature = "hot_code_loading")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hot_code_loading")))]
    /// Get the current version of a module.
    pub fn module_version(&self, module_name: &str) -> Option<String> {
        let cm = self.code_manager.lock().unwrap();
        cm.current_version(module_name)
    }

    #[cfg(feature = "hot_code_loading")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hot_code_loading")))]
    /// List all loaded versions for a module.
    pub fn module_versions(&self, module_name: &str) -> Vec<String> {
        let cm = self.code_manager.lock().unwrap();
        cm.list_versions(module_name)
    }

    #[cfg(feature = "hot_code_loading")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hot_code_loading")))]
    /// Get a reference to the actor factory for this runtime.
    pub fn actor_factory(&self) -> &ActorFactory {
        self.factory.as_ref().expect("factory not initialized")
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

// ============================================================================
// Configuration and statistics
// ============================================================================

/// Runtime configuration.
#[must_use]
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Configuration for the compute scheduler (thread pool for CPU tasks).
    pub compute: ComputeConfig,
    /// Maximum number of messages an actor mailbox can hold before
    /// [`Runtime::send`] returns [`RuntimeError::MailboxFull`]. Default: 1000.
    pub mailbox_capacity: usize,
    /// Maximum time to wait during shutdown for actors to drain.
    /// Default: 5 seconds.
    pub shutdown_timeout: Duration,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            compute: ComputeConfig::default(),
            mailbox_capacity: 1000,
            shutdown_timeout: Duration::from_secs(5),
        }
    }
}

/// Snapshot of runtime statistics.
#[must_use]
#[derive(Debug, Clone)]
pub struct RuntimeStats {
    /// Number of currently alive actors.
    pub actor_count: usize,
    /// Total number of requests sent through the runtime.
    pub request_count: u64,
}

// ============================================================================
// RuntimeSender — clonable handle for external threads
// ============================================================================

/// Clonable handle for sending messages to actors from external threads.
#[must_use]
#[derive(Clone)]
pub struct RuntimeSender {
    pub(crate) senders: Arc<RwLock<HashMap<ActorId, crossbeam_channel::Sender<MessageEnvelope>>>>,
    pub(crate) request_counter: Arc<AtomicU64>,
}

impl RuntimeSender {
    /// Send a fire-and-forget message to an actor. Non-blocking.
    pub fn send<M: Send + 'static>(&self, target: ActorId, message: M) -> Result<(), RuntimeError> {
        let senders = self
            .senders
            .read()
            .map_err(|_| RuntimeError::RuntimeStopped)?;
        let sender = senders
            .get(&target)
            .ok_or(RuntimeError::ActorNotFound(target))?;
        sender
            .try_send(MessageEnvelope::Message(Box::new(message)))
            .map_err(|e| match e {
                crossbeam_channel::TrySendError::Full(_) => RuntimeError::MailboxFull(target),
                crossbeam_channel::TrySendError::Disconnected(_) => RuntimeError::RuntimeStopped,
            })
    }

    /// Send a request to an actor and get a handle to receive the reply.
    pub fn request<M: Send + 'static>(
        &self,
        target: ActorId,
        message: M,
    ) -> Result<RequestHandle, RuntimeError> {
        let request_id = self
            .request_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (reply_tx, reply_rx) = crossbeam_channel::bounded(1);

        let senders = self
            .senders
            .read()
            .map_err(|_| RuntimeError::RuntimeStopped)?;
        let sender = senders
            .get(&target)
            .ok_or(RuntimeError::ActorNotFound(target))?;

        sender
            .try_send(MessageEnvelope::Request {
                _request_id: request_id,
                payload: Box::new(message),
                reply_sender: reply_tx,
            })
            .map_err(|e| match e {
                crossbeam_channel::TrySendError::Full(_) => RuntimeError::MailboxFull(target),
                crossbeam_channel::TrySendError::Disconnected(_) => RuntimeError::RuntimeStopped,
            })?;

        Ok(RequestHandle {
            id: request_id,
            receiver: reply_rx,
        })
    }
}

// ============================================================================
// Futures
// ============================================================================

/// Future that completes after a specified duration.
#[derive(Debug)]
pub struct SleepFuture {
    receiver: crossbeam_channel::Receiver<()>,
}

impl SleepFuture {
    pub fn new(duration: Duration) -> Self {
        let (tx, rx) = crossbeam_channel::bounded(1);
        std::thread::spawn(move || {
            std::thread::sleep(duration);
            let _ = tx.send(());
        });
        Self { receiver: rx }
    }
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.receiver.try_recv() {
            Ok(()) => Poll::Ready(()),
            Err(crossbeam_channel::TryRecvError::Empty) => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => Poll::Ready(()),
        }
    }
}

/// Future that completes with a timeout.
#[derive(Debug)]
pub struct TimeoutFuture<F> {
    future: F,
    receiver: crossbeam_channel::Receiver<()>,
}

impl<F> TimeoutFuture<F> {
    pub fn new(duration: Duration, future: F) -> Self {
        let (tx, rx) = crossbeam_channel::bounded(1);
        std::thread::spawn(move || {
            std::thread::sleep(duration);
            let _ = tx.send(());
        });
        Self {
            future,
            receiver: rx,
        }
    }
}

impl<F: Future> Future for TimeoutFuture<F> {
    type Output = Result<F::Output, TaskError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Ok(()) = self.receiver.try_recv() {
            return Poll::Ready(Err(TaskError::Timeout));
        }
        let this = unsafe { self.get_unchecked_mut() };
        let future = unsafe { Pin::new_unchecked(&mut this.future) };
        match future.poll(cx) {
            Poll::Ready(output) => Poll::Ready(Ok(output)),
            Poll::Pending => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}
