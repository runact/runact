use crate::actor::context::ActorContext;
use crate::actor::error::ActorError;

/// Actor trait — the fundamental unit of computation.
pub trait Actor: Send + 'static {
    /// Message type this actor handles.
    type Message: Send + 'static;

    /// Handle a message.
    ///
    /// Called for each message received by the actor.
    /// Must return quickly — don't block or loop forever.
    fn handle(
        &mut self,
        message: Self::Message,
        context: &mut ActorContext,
    ) -> Result<(), ActorError>;
}
