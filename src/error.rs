use thiserror::Error;

/// Runtime errors.
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("actor not found: {0}")]
    ActorNotFound(crate::actor::ActorId),

    #[error("mailbox full for actor: {0}")]
    MailboxFull(crate::actor::ActorId),

    #[error("actor already exists: {0}")]
    ActorExists(crate::actor::ActorId),

    #[error("shutdown timeout")]
    ShutdownTimeout,

    #[error("runtime stopped")]
    RuntimeStopped,
}
