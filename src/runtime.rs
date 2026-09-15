use std::collections::HashMap;
use std::sync::{Arc, RwLock, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use crate::actor::{Actor, ActorId, ActorContext};
use crate::scheduler::{Scheduler, ReductionCounter, MAX_REDUCTIONS};
use crate::supervision::{Supervisor, RestartStrategy, ChildSpec};
use crate::compute::{ComputeScheduler, ComputeConfig};
use crate::error::RuntimeError;

use crate::timer::{TimerService, TimerId};

/// Envelope wrapping messages sent to actors.
pub(crate) enum MessageEnvelope {
    Message(Box<dyn std::any::Any + Send>),
    Request {
        _request_id: u64,
        payload: Box<dyn std::any::Any + Send>,
        reply_sender: crossbeam_channel::Sender<Box<dyn std::any::Any + Send>>,
    },
}

/// Handle to a pending request. Receives the reply asynchronously.
#[must_use]
pub struct RequestHandle {
    id: u64,
    receiver: crossbeam_channel::Receiver<Box<dyn std::any::Any + Send>>,
}

impl RequestHandle {
    /// Get the request's correlation ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Block until a reply is received.
    pub fn recv(self) -> Result<Box<dyn std::any::Any + Send>, RuntimeError> {
        self.receiver
            .recv()
            .map_err(|_| RuntimeError::RuntimeStopped)
    }

    /// Try to receive a reply without blocking.
    pub fn try_recv(&self) -> Result<Box<dyn std::any::Any + Send>, RuntimeError> {
        self.receiver
            .try_recv()
            .map_err(|_| RuntimeError::RuntimeStopped)
    }

    /// Receive a reply with a timeout.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Box<dyn std::any::Any + Send>, RuntimeError> {
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
    /// Optional human-readable name (e.g., `"supervisor"`).
    pub name: Option<String>,
    /// Current number of messages queued in the actor's mailbox.
    pub mailbox_depth: usize,
}

/// Top-level runtime coordinator.
#[must_use]
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
    /// Create a new runtime.
    pub fn new() -> Result<Self, RuntimeError> {
        Self::with_config(RuntimeConfig::default())
    }

    /// Create a new runtime with configuration.
    pub fn with_config(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let scheduler = Scheduler::new();
        let compute = ComputeScheduler::new(config.compute)
            .map_err(|_| RuntimeError::RuntimeStopped)?;

        let senders = Arc::new(RwLock::new(HashMap::new()));
        let timers = TimerService::new(senders.clone());

        Ok(Self {
            scheduler,
            compute,
            timers,
            actors: Arc::new(RwLock::new(HashMap::new())),
            senders,
            request_counter: AtomicU64::new(0),
            stop: Arc::new(AtomicBool::new(false)),
            mailbox_capacity: config.mailbox_capacity,
            shutdown_timeout: config.shutdown_timeout,
        })
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

        // Store the sender so we can route messages to this actor
        self.senders
            .write()
            .map_err(|_| RuntimeError::RuntimeStopped)?
            .insert(id, tx.clone());

        let mut ctx = ActorContext::new(id, tx);
        ctx.set_senders(self.senders.clone());
        ctx.set_timer_handle(self.timers.handle());
        if let Some(compute_tx) = self.compute.sender() {
            ctx.set_compute_sender(compute_tx);
        }
        let stop = self.stop.clone();
        let reductions = ReductionCounter::new();

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
                                MessageEnvelope::Request { payload, reply_sender, .. } => {
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
                            MessageEnvelope::Request { payload, reply_sender, .. } => {
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
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        Ok(id)
    }

    /// Send a fire-and-forget message to an actor (non-blocking).
    ///
    /// Uses `try_send` internally — returns [`RuntimeError::MailboxFull`] if the
    /// actor's mailbox is at capacity. For external threads that can afford to
    /// block, use [`Runtime::send_blocking`] instead.
    pub fn send<M: Send + 'static>(&self, target: ActorId, message: M) -> Result<(), RuntimeError> {
        tracing::debug!(target_id = %target, "send");
        let senders = self.senders.read().map_err(|_| RuntimeError::RuntimeStopped)?;
        let sender = senders.get(&target).ok_or(RuntimeError::ActorNotFound(target))?;
        sender
            .try_send(MessageEnvelope::Message(Box::new(message)))
            .map_err(|e| match e {
                crossbeam_channel::TrySendError::Full(_) => RuntimeError::MailboxFull(target),
                crossbeam_channel::TrySendError::Disconnected(_) => RuntimeError::RuntimeStopped,
            })
    }

    /// Send a fire-and-forget message to an actor, blocking until the mailbox accepts it.
    ///
    /// **For use by external threads only.** Actors must never call this — it would
    /// block the scheduler worker thread. Inside actors, use [`ActorContext::send_to`].
    pub fn send_blocking<M: Send + 'static>(&self, target: ActorId, message: M) -> Result<(), RuntimeError> {
        tracing::debug!(target_id = %target, "send_blocking");
        let senders = self.senders.read().map_err(|_| RuntimeError::RuntimeStopped)?;
        let sender = senders.get(&target).ok_or(RuntimeError::ActorNotFound(target))?;
        sender
            .send(MessageEnvelope::Message(Box::new(message)))
            .map_err(|_| RuntimeError::RuntimeStopped)
    }

    /// Send a request to an actor and get a handle to receive the reply.
    ///
    /// The message is enqueued with a one-shot reply channel. The actor can
    /// reply using [`ActorContext::reply`]. The returned [`RequestHandle`]
    /// provides `recv()`, `try_recv()`, and `recv_timeout()` to receive the reply.
    pub fn request<M: Send + 'static>(&self, target: ActorId, message: M) -> Result<RequestHandle, RuntimeError> {
        let request_id = self.request_counter.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(target_id = %target, request_id, "request");
        let (reply_tx, reply_rx) = crossbeam_channel::bounded(1);

        let senders = self.senders.read().map_err(|_| RuntimeError::RuntimeStopped)?;
        let sender = senders.get(&target).ok_or(RuntimeError::ActorNotFound(target))?;

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
    ///
    /// This is meant for external threads, NOT for use inside actors
    /// (which would block the scheduler worker).
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
                reply.downcast::<R>().map(|b| *b).map_err(|_| RuntimeError::RuntimeStopped)
            }
            None => {
                let reply = handle.recv()?;
                reply.downcast::<R>().map(|b| *b).map_err(|_| RuntimeError::RuntimeStopped)
            }
        }
    }

    /// Spawn a supervisor that manages child actor lifecycles.
    ///
    /// The supervisor tracks children defined by `children` and restarts them
    /// according to `strategy` when they crash.
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

    /// List all actors.
    pub fn list_actors(&self) -> Result<Vec<ActorInfo>, RuntimeError> {
        let actors = self.actors.read().map_err(|_| RuntimeError::RuntimeStopped)?;
        Ok(actors.values().cloned().collect())
    }

    /// Inspect an actor.
    pub fn inspect_actor(&self, id: ActorId) -> Result<Option<ActorInfo>, RuntimeError> {
        let actors = self.actors.read().map_err(|_| RuntimeError::RuntimeStopped)?;
        Ok(actors.get(&id).cloned())
    }

    /// Get a reference to the compute scheduler.
    pub fn compute(&self) -> &ComputeScheduler {
        &self.compute
    }

    /// Schedule a one-shot timer that sends a message to an actor after a delay.
    ///
    /// Returns a [`TimerId`] that can be passed to [`Runtime::cancel_timer`].
    pub fn schedule_timer<M: Send + 'static>(
        &self,
        duration: Duration,
        target: ActorId,
        message: M,
    ) -> TimerId {
        self.timers.schedule(duration, target, Box::new(message))
    }

    /// Schedule a periodic timer that sends a message to an actor at intervals.
    ///
    /// The message must be `Clone + Send + Sync` because it is cloned for
    /// each tick. Returns a [`TimerId`] for cancellation.
    pub fn schedule_interval<M: Clone + Send + Sync + 'static>(
        &self,
        interval: Duration,
        target: ActorId,
        message: M,
    ) -> TimerId {
        let message = Arc::new(message);
        self.timers.schedule_interval(interval, target, Box::new(move || {
            Box::new((*message).clone()) as Box<dyn std::any::Any + Send>
        }))
    }

    /// Get runtime statistics.
    pub fn stats(&self) -> RuntimeStats {
        let actors = self.actors.read().unwrap_or_else(|e| e.into_inner());
        RuntimeStats {
            actor_count: actors.len(),
            request_count: self.request_counter.load(Ordering::Relaxed),
        }
    }

    /// Cancel a scheduled timer.
    pub fn cancel_timer(&self, timer_id: TimerId) {
        self.timers.cancel(timer_id);
    }

    /// Gracefully shut down the runtime.
    ///
    /// Sets the stop flag, waits up to `shutdown_timeout` for actors to drain
    /// their remaining messages, then clears all internal state.
    pub fn shutdown(&mut self) -> Result<(), RuntimeError> {
        self.stop.store(true, Ordering::SeqCst);

        let deadline = Instant::now() + self.shutdown_timeout;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        tracing::info!(timeout_ms = self.shutdown_timeout.as_millis() as u64, "runtime shutdown");

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
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

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
