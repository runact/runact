//! Runact — a Rust-native actor runtime.
//!
//! Runact combines BEAM-style lightweight processes, messaging, scheduling
//! and supervision with Rust's ownership model.
//!
//! # Quick Start
//!
//! ```rust
//! use runact::{Actor, ActorId, ActorContext, ActorError, Runtime};
//!
//! struct Greeter;
//!
//! impl Actor for Greeter {
//!     type Message = String;
//!
//!     fn handle(&mut self, msg: String, ctx: &mut ActorContext) -> Result<(), ActorError> {
//!         println!("Hello, {}!", msg);
//!         Ok(())
//!     }
//! }
//!
//! let mut runtime = Runtime::new().unwrap();
//! let id = runtime.spawn(Greeter).unwrap();
//! runtime.send(id, "world".to_string()).unwrap();
//! ```
//!
//! # Core Concepts
//!
//! - **Actors** — isolated units of state and behavior that communicate via messages.
//! - **Runtime** — the top-level coordinator that spawns actors and routes messages.
//! - **Supervision** — fault tolerance via configurable restart strategies.
//! - **Compute** — a thread pool for CPU-intensive work off the actor scheduler.
//! - **Timers** — scheduled one-shot and periodic message delivery.
//! - **Resources** — type-safe capability-based resource management.

pub mod actor;
pub mod timer;
pub mod resource;
pub mod error;

mod runtime;
mod scheduler;
mod mailbox;
mod compute;

pub use actor::{Actor, ActorId, ActorContext, ActorError};
pub use scheduler::Scheduler;
pub use supervision::{Supervisor, RestartStrategy, ChildSpec, RestartPolicy};
pub use compute::{ComputeScheduler, ComputeConfig, ComputeHandle, ComputeError};
pub use timer::{Timer, TimerConfig, TimerId};
pub use error::RuntimeError;
pub use runtime::{Runtime, RuntimeConfig, RuntimeStats, RequestHandle, ActorInfo};
pub use resource::{ResourceHandle, Capability, ResourceRegistry};

mod supervision;
