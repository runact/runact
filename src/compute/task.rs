use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

/// A unit of CPU work submitted to the compute scheduler.
pub struct Task {
    /// Unique task identifier.
    pub id: TaskId,
    /// The closure to execute on a worker thread.
    pub job: Box<dyn FnOnce() -> Box<dyn std::any::Any + Send> + Send>,
    /// Channel to send the result back to the caller.
    pub result_sender: crossbeam_channel::Sender<ComputeResult>,
    /// When the task was created (for timeout tracking).
    pub created_at: Instant,
    /// Cancellation flag — set by [`ComputeHandle::cancel`](crate::compute::ComputeHandle::cancel).
    pub cancelled: Arc<AtomicBool>,
}

/// Unique task identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
    /// Create a new TaskId.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a compute task execution.
#[derive(Debug)]
pub enum ComputeResult {
    /// Task completed successfully with a boxed result.
    Ok(Box<dyn std::any::Any + Send>),
    /// Task failed with a compute error (e.g., scheduler shutdown).
    Err(ComputeError),
    /// The task panicked. Contains the panic message if available.
    Panic(String),
    /// The task exceeded its timeout (reserved for future use).
    Timeout,
    /// The task was cancelled before execution.
    Cancelled,
}

/// Errors that can occur during compute task submission or execution.
#[derive(Debug)]
pub enum ComputeError {
    /// The compute queue is at capacity (not currently used with unbounded channel).
    QueueFull,
    /// The compute scheduler has been shut down.
    SchedulerShutdown,
    /// A worker thread panicked while executing the task.
    WorkerPanic(String),
}

impl std::fmt::Display for ComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputeError::QueueFull => write!(f, "compute queue full"),
            ComputeError::SchedulerShutdown => write!(f, "compute scheduler shutdown"),
            ComputeError::WorkerPanic(msg) => write!(f, "worker panic: {}", msg),
        }
    }
}

impl std::error::Error for ComputeError {}
