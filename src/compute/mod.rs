//! Compute — CPU-intensive task scheduler.

mod scheduler;
mod task;
mod handle;

pub use scheduler::{ComputeScheduler, ComputeConfig};
pub(crate) use task::{Task, TaskId};
pub use task::ComputeError;
pub use handle::ComputeHandle;
