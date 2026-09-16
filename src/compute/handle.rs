use crate::compute::task::{ComputeError, ComputeResult};
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Handle to a submitted compute task. Used to retrieve the result or
/// cancel the task.
#[must_use]
pub struct ComputeHandle<T> {
    receiver: crossbeam_channel::Receiver<ComputeResult>,
    cancelled: Arc<AtomicBool>,
    _phantom: PhantomData<T>,
}

impl<T> ComputeHandle<T> {
    pub(crate) fn new(
        receiver: crossbeam_channel::Receiver<ComputeResult>,
        cancelled: Arc<AtomicBool>,
    ) -> Self {
        Self {
            receiver,
            cancelled,
            _phantom: PhantomData,
        }
    }

    /// Request cancellation of this compute task.
    ///
    /// The worker thread checks this flag before and after execution.
    /// If set before: the task is skipped entirely (`ComputeResult::Cancelled`).
    /// If set after: the result is discarded.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Returns `true` if cancellation has been requested for this task.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn map_result(result: ComputeResult) -> Result<T, ComputeError>
    where
        T: 'static,
    {
        match result {
            ComputeResult::Ok(value) => value
                .downcast::<T>()
                .map(|v| *v)
                .map_err(|_| ComputeError::WorkerPanic("Type mismatch".to_string())),
            ComputeResult::Err(e) => Err(e),
            ComputeResult::Panic(msg) => Err(ComputeError::WorkerPanic(msg)),
            ComputeResult::Timeout => Err(ComputeError::WorkerPanic("Task timed out".to_string())),
            ComputeResult::Cancelled => {
                Err(ComputeError::WorkerPanic("Task cancelled".to_string()))
            }
        }
    }

    /// Try to receive the result without blocking.
    ///
    /// Returns `Some(Ok(T))` if the task completed successfully,
    /// `Some(Err(...))` on failure/cancel/timeout, or `None` if still running.
    pub fn try_recv(&self) -> Option<Result<T, ComputeError>>
    where
        T: 'static,
    {
        match self.receiver.try_recv() {
            Ok(result) => Some(Self::map_result(result)),
            Err(crossbeam_channel::TryRecvError::Empty) => None,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                Some(Err(ComputeError::SchedulerShutdown))
            }
        }
    }

    /// Block until the compute task completes and return the result.
    pub fn recv(&self) -> Result<T, ComputeError>
    where
        T: 'static,
    {
        match self.receiver.recv() {
            Ok(result) => Self::map_result(result),
            Err(_) => Err(ComputeError::SchedulerShutdown),
        }
    }

    /// Block until the compute task completes or the timeout elapses.
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Result<T, ComputeError>
    where
        T: 'static,
    {
        match self.receiver.recv_timeout(timeout) {
            Ok(result) => Self::map_result(result),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                Err(ComputeError::WorkerPanic("Timeout".to_string()))
            }
            Err(_) => Err(ComputeError::SchedulerShutdown),
        }
    }
}
