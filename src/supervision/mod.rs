//! Supervision — BEAM-inspired supervision tree.

mod supervisor;
mod strategy;
mod child;
mod supervisor_actor;

pub use supervisor::Supervisor;
pub use strategy::RestartStrategy;
pub use child::{ChildSpec, RestartPolicy};
