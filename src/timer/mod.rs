//! Timer — scheduled message delivery.

mod types;
mod service;

pub use types::{Timer, TimerConfig};
pub use service::{TimerService, TimerHandle, TimerId};
