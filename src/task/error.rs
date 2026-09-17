//! Error types for asynchronous tasks.

use thiserror::Error;

/// Errors that can occur while running or retrieving an async task.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TaskError {
    /// The task's future panicked while being polled. Contains the panic
    /// message if one could be extracted.
    #[error("task panicked: {0}")]
    Panic(String),

    /// The executor has been shut down before the task could complete.
    #[error("executor shut down")]
    ExecutorShutdown,

    /// The task did not complete before the receive timeout elapsed.
    #[error("task did not complete before the timeout")]
    Timeout,
}

/// Result of an async task execution, sent from the executor worker to the
/// [`TaskHandle`](crate::TaskHandle).
#[derive(Debug)]
pub(crate) enum TaskResult {
    /// Task completed successfully with a boxed output value.
    Ok(Box<dyn std::any::Any + Send>),
    /// Task panicked while being polled. Contains the panic message.
    Panic(String),
    /// Executor shut down before the task completed.
    Shutdown,
}
