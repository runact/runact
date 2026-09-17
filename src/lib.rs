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
//! - **Async tasks** — a native executor for standard Rust `Futures` with task handles.
//! - **Timers** — scheduled one-shot and periodic message delivery.
//! - **Resources** — type-safe capability-based resource management.

pub mod actor;
pub mod error;
pub mod resource;
pub mod timer;

mod compute;
mod mailbox;
mod runtime;
mod scheduler;
mod task;

pub use actor::{Actor, ActorContext, ActorError, ActorId};
pub use compute::{ComputeConfig, ComputeError, ComputeHandle, ComputeScheduler};
pub use error::RuntimeError;
pub use resource::{Capability, ResourceHandle, ResourceRegistry};
pub use runtime::{ActorInfo, RequestHandle, Runtime, RuntimeConfig, RuntimeStats};
pub use scheduler::Scheduler;
pub use supervision::{ChildSpec, RestartPolicy, RestartStrategy, Supervisor};
pub use task::{TaskError, TaskHandle, TaskId};
pub use timer::{Timer, TimerConfig, TimerId};

mod supervision;
