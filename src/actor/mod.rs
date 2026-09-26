//! Actor trait and ActorId.

pub mod context;
mod error;
mod id;
mod trait_def;

pub use context::ActorContext;
#[cfg(feature = "hot_code_loading")]
pub use context::SpawnRequest;
pub use error::ActorError;
pub use id::ActorId;
pub use trait_def::Actor;
