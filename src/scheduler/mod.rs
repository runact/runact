//! BEAM-style scheduler with work stealing.

mod reduction;
mod run_queue;
mod work_stealing;
mod worker;

pub(crate) use reduction::{MAX_REDUCTIONS, ReductionCounter};
pub use work_stealing::Scheduler;
