//! Handle to a spawned async task.

use super::error::{TaskError, TaskResult};
use super::id::TaskId;
use std::sync::Mutex;
use std::time::Duration;

/// Handle to a spawned async task.
///
/// Receives the terminal outcome of the task: its output value, a panic
/// message, or the executor shutting down. The outcome is cached on first
/// delivery, so a completed task can be queried more than once (the output
/// must be `Clone` for that reason).
#[must_use]
#[derive(Debug)]
pub struct TaskHandle<T> {
    id: TaskId,
    receiver: crossbeam_channel::Receiver<TaskResult>,
    outcome: Mutex<Option<Result<T, TaskError>>>,
}

impl<T> TaskHandle<T> {
    pub(crate) fn new(id: TaskId, receiver: crossbeam_channel::Receiver<TaskResult>) -> Self {
        Self {
            id,
            receiver,
            outcome: Mutex::new(None),
        }
    }

    /// The unique identifier of the underlying task.
    pub fn id(&self) -> TaskId {
        self.id
    }

    /// Block until the task completes and return its output.
    pub fn recv(&self) -> Result<T, TaskError>
    where
        T: Clone + 'static,
    {
        if let Some(outcome) = self.cached() {
            return outcome;
        }
        match self.receiver.recv() {
            Ok(result) => self.store(result),
            Err(_) => Err(TaskError::ExecutorShutdown),
        }
    }

    /// Receive the task's outcome without blocking.
    ///
    /// Returns `Some(...)` once the task has terminated, `None` while it is
    /// still pending.
    pub fn try_recv(&self) -> Option<Result<T, TaskError>>
    where
        T: Clone + 'static,
    {
        if let Some(outcome) = self.cached() {
            return Some(outcome);
        }
        match self.receiver.try_recv() {
            Ok(result) => Some(self.store(result)),
            Err(crossbeam_channel::TryRecvError::Empty) => None,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                Some(Err(TaskError::ExecutorShutdown))
            }
        }
    }

    /// Block until the task completes or `timeout` elapses.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, TaskError>
    where
        T: Clone + 'static,
    {
        if let Some(outcome) = self.cached() {
            return outcome;
        }
        match self.receiver.recv_timeout(timeout) {
            Ok(result) => self.store(result),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => Err(TaskError::Timeout),
            Err(_) => Err(TaskError::ExecutorShutdown),
        }
    }

    /// Clone of the cached outcome, if the task already terminated.
    fn cached(&self) -> Option<Result<T, TaskError>>
    where
        T: Clone,
    {
        self.outcome
            .lock()
            .expect("task handle outcome mutex poisoned")
            .clone()
    }

    /// Downcast the boxed result into `T` and cache it for later queries.
    fn store(&self, result: TaskResult) -> Result<T, TaskError>
    where
        T: Clone + 'static,
    {
        let outcome = match result {
            TaskResult::Ok(value) => value
                .downcast::<T>()
                .map(|v| *v)
                .map_err(|_| TaskError::Panic("output type mismatch".to_string())),
            TaskResult::Panic(message) => Err(TaskError::Panic(message)),
            TaskResult::Shutdown => Err(TaskError::ExecutorShutdown),
        };
        *self
            .outcome
            .lock()
            .expect("task handle outcome mutex poisoned") = Some(outcome.clone());
        outcome
    }
}
