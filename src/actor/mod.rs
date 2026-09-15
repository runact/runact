//! Actor trait and ActorId.

mod id;
mod trait_def;
mod context;
mod error;

pub use id::ActorId;
pub use trait_def::Actor;
pub use context::ActorContext;
pub use error::ActorError;
