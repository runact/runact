use thiserror::Error;

/// Actor errors.
#[derive(Debug, Error)]
pub enum ActorError {
    #[error("handler error: {0}")]
    Handler(String),

    #[error("cancelled")]
    Cancelled,

    #[error("timeout")]
    Timeout,

    #[error("panic: {0}")]
    Panic(String),
}
