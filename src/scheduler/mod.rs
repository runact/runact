//! BEAM-style scheduler with work stealing.

mod work_stealing;
mod worker;
mod run_queue;
mod reduction;

pub use work_stealing::Scheduler;
pub(crate) use reduction::{ReductionCounter, MAX_REDUCTIONS};
