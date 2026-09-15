use std::collections::HashMap;
use std::sync::{Arc, atomic::AtomicBool, RwLock};
use crate::actor::id::ActorId;
use crate::error::RuntimeError;
use crate::runtime::MessageEnvelope;
use crate::timer::{TimerHandle, TimerId};
use crate::compute::{ComputeHandle, ComputeError, Task, TaskId};

type SenderMap = Arc<RwLock<HashMap<ActorId, crossbeam_channel::Sender<MessageEnvelope>>>>;

type ReplySender = crossbeam_channel::Sender<Box<dyn std::any::Any + Send>>;

/// Context provided to actor handlers.
pub struct ActorContext {
    actor_id: ActorId,
    _sender: crossbeam_channel::Sender<MessageEnvelope>,
    senders: Option<SenderMap>,
    reply_sender: Option<ReplySender>,
    compute_sender: Option<crossbeam_channel::Sender<Task>>,
    timer_handle: Option<TimerHandle>,
}

impl ActorContext {
    pub(crate) fn new(
        actor_id: ActorId,
        sender: crossbeam_channel::Sender<MessageEnvelope>,
    ) -> Self {
        Self {
            actor_id,
            _sender: sender,
            senders: None,
            reply_sender: None,
            compute_sender: None,
            timer_handle: None,
        }
    }

    pub(crate) fn set_senders(&mut self, senders: SenderMap) {
        self.senders = Some(senders);
    }

    pub(crate) fn set_reply_sender(&mut self, reply_sender: Option<ReplySender>) {
        self.reply_sender = reply_sender;
    }

    pub(crate) fn set_compute_sender(
        &mut self,
        compute_sender: crossbeam_channel::Sender<Task>,
    ) {
        self.compute_sender = Some(compute_sender);
    }

    pub(crate) fn set_timer_handle(&mut self, timer_handle: TimerHandle) {
        self.timer_handle = Some(timer_handle);
    }

    /// Get the ID of the current actor.
    pub fn actor_id(&self) -> ActorId {
        self.actor_id
    }

    /// Send a fire-and-forget message to another actor.
    ///
    /// This is the actor-to-actor messaging primitive. Messages are queued
    /// in the target actor's mailbox and delivered asynchronously.
    ///
    /// # Errors
    ///
    /// - [`RuntimeError::ActorNotFound`] if `target` doesn't exist.
    /// - [`RuntimeError::RuntimeStopped`] if the runtime has shut down.
    pub fn send_to<M: Send + 'static>(
        &self,
        target: ActorId,
        message: M,
    ) -> Result<(), crate::error::RuntimeError> {
        let senders = self.senders.as_ref().ok_or(crate::error::RuntimeError::RuntimeStopped)?;
        let senders = senders.read().map_err(|_| crate::error::RuntimeError::RuntimeStopped)?;
        let sender = senders.get(&target).ok_or(crate::error::RuntimeError::ActorNotFound(target))?;
        sender
            .send(MessageEnvelope::Message(Box::new(message)))
            .map_err(|_| crate::error::RuntimeError::RuntimeStopped)
    }

    /// Reply to the sender of the current request.
    ///
    /// If the current message was sent via [`crate::Runtime::request`], this sends
    /// the reply back to the caller. If the message was fire-and-forget,
    /// this is a no-op.
    pub fn reply<M: Send + 'static>(&self, message: M) -> Result<(), crate::error::RuntimeError> {
        if let Some(ref reply_tx) = self.reply_sender {
            reply_tx
                .send(Box::new(message))
                .map_err(|_| crate::error::RuntimeError::RuntimeStopped)
        } else {
            Ok(())
        }
    }

    /// Returns `true` if the current message was sent as a request
    /// (i.e., the sender expects a reply via [`ActorContext::reply`]).
    pub fn is_request(&self) -> bool {
        self.reply_sender.is_some()
    }

    /// Submit a CPU-intensive task to the compute pool.
    ///
    /// Returns a `ComputeHandle` that can be used to receive the result.
    /// The actor should store this handle and poll it later (e.g., on the next message).
    /// This is non-blocking — the task runs on a compute worker thread.
    pub fn spawn_compute<F, T>(
        &self,
        job: F,
    ) -> Result<ComputeHandle<T>, ComputeError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let sender = self.compute_sender.as_ref().ok_or(ComputeError::SchedulerShutdown)?;
        let (result_sender, result_receiver) = crossbeam_channel::bounded(1);
        let cancelled = Arc::new(AtomicBool::new(false));

        let task = Task {
            id: TaskId::new(),
            job: Box::new(move || Box::new(job()) as Box<dyn std::any::Any + Send>),
            result_sender,
            created_at: std::time::Instant::now(),
            cancelled: cancelled.clone(),
        };

        sender
            .send(task)
            .map_err(|_| ComputeError::SchedulerShutdown)?;

        Ok(ComputeHandle::new(result_receiver, cancelled))
    }

    /// Schedule a one-shot timer that sends a message to this actor after a delay.
    ///
    /// Returns a [`TimerId`] that can be passed to [`ActorContext::cancel_timer`]
    /// to cancel the timer before it fires.
    pub fn schedule_timer<M: Send + 'static>(
        &self,
        duration: std::time::Duration,
        message: M,
    ) -> Result<TimerId, RuntimeError> {
        let handle = self.timer_handle.as_ref().ok_or(RuntimeError::RuntimeStopped)?;
        Ok(handle.schedule_timer(duration, self.actor_id, message))
    }

    /// Schedule a periodic timer that sends a message to this actor at intervals.
    ///
    /// The message must be `Clone + Send + Sync` because it is cloned for
    /// each tick. Returns a [`TimerId`] for cancellation.
    pub fn schedule_interval<M: Clone + Send + Sync + 'static>(
        &self,
        interval: std::time::Duration,
        message: M,
    ) -> Result<TimerId, RuntimeError> {
        let handle = self.timer_handle.as_ref().ok_or(RuntimeError::RuntimeStopped)?;
        Ok(handle.schedule_interval(interval, self.actor_id, message))
    }

    /// Cancel a previously scheduled timer.
    pub fn cancel_timer(&self, timer_id: TimerId) {
        if let Some(ref handle) = self.timer_handle {
            handle.cancel_timer(timer_id);
        }
    }
}
