//! Compute — CPU-intensive task scheduler.

mod handle;
mod scheduler;
mod task;

pub use handle::ComputeHandle;
pub use scheduler::{ComputeConfig, ComputeScheduler};
pub use task::ComputeError;
pub(crate) use task::{Task, TaskId};
