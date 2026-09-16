//! Actor trait and ActorId.

mod context;
mod error;
mod id;
mod trait_def;

pub use context::ActorContext;
pub use error::ActorError;
pub use id::ActorId;
pub use trait_def::Actor;
