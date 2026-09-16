//! Timer — scheduled message delivery.

mod service;
mod types;

pub use service::{TimerHandle, TimerId, TimerService};
pub use types::{Timer, TimerConfig};
