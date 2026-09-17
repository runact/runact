//! Native async tasks: a small executor for standard Rust futures.
//!
//! Runact schedules asynchronous work; I/O libraries define it
//! (see `docs/async-runtime.md`). This module is the executor half of that
//! boundary: a dedicated worker pool polls [`TaskCell`]s driven by a
//! Runact-specific waker with wake deduplication, and every task is
//! reachable through a [`TaskHandle`] for the lifetime of the runtime.
//!
//! Pending futures consume no worker execution time; a wake re-queues the
//! task exactly once per `IDLE → SCHEDULED` transition. Shutdown sweeps the
//! live-task registry, so a pending handle always resolves deterministically
//! instead of blocking forever.

mod cell;
mod error;
mod executor;
mod handle;
mod id;

pub use error::TaskError;
pub use handle::TaskHandle;
pub use id::TaskId;

pub(crate) use executor::Executor;
