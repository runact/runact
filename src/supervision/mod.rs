//! Supervision — BEAM-inspired supervision tree.

mod child;
mod strategy;
mod supervisor;
mod supervisor_actor;

pub use child::{ChildSpec, RestartPolicy};
pub use strategy::RestartStrategy;
pub use supervisor::Supervisor;
