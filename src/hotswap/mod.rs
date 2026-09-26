//! Hot code loading for Runact — Pattern 2 (Dynamic Supervision)
//!
//! This module provides Erlang/OTP-like hot code loading capabilities where a
//! supervisor actor spawns child actors per work unit. New child processes
//! automatically use the latest code version, while existing processes keep
//! their original code until explicitly upgraded.
//!
//! ## Architecture
//!
//! ```text
//! Runtime
//! ├── CodeManager (version registry)
//! ├── ActorFactory (spawn_dynamic always uses current version)
//! └── Supervisors → spawn children per work unit
//!                    ├── Child v1.0 (old code)
//!                    └── Child v1.1 (new code, auto-selected)
//! ```
//!
//! When `prepare_dynamic_spawn()` is called, the factory always retrieves the current
//! version's vtable, so new child actors automatically use the latest code.
//!
//! ## Pattern 2: Dynamic Supervision
//!
//! The [`DynamicSupervisor`] implements Pattern 2: it receives work items and
//! each work item triggers a new child actor spawn. Because the supervisor uses
//! `ActorFactory::prepare_dynamic_spawn()` — which always selects the current
//! version — **new child actors spawned after a hot code upgrade automatically
//! run the new code version**.

pub mod codemgr;
pub mod factory;
pub mod loader;
pub mod migration;
pub mod types;

#[cfg(feature = "hot_code_loading")]
pub mod dynamic_supervisor;

pub use codemgr::{CodeManager, CodePath};
pub use factory::ActorFactory;
pub use loader::{DynamicLibrary, LibraryLoader};
pub use migration::{MigrationFn, MigrationRegistry, StateMigration};
pub use types::{
    ActorSpec, ActorVTable, DeserializeFn, DropFn, MessageHandler, MigrateFn, SerializeFn,
    VersionedActorRef, WorkItem, WorkerError, WorkerMessage, WorkerResponder,
};

#[cfg(feature = "hot_code_loading")]
pub use dynamic_supervisor::DynamicSupervisor;

use crate::error::RuntimeError;

/// Hot-swappable actor trait.
///
/// Actors implementing this trait can be upgraded at runtime.
/// The actor must be able to serialize/deserialize its state and migrate
/// between versions.
pub trait HotSwappable: crate::actor::Actor {
    /// Semantic version of this actor implementation (e.g. "1.0.0")
    const VERSION: &'static str;

    /// Serialize internal state for migration/hotswap.
    fn serialize(&self) -> Vec<u8>;

    /// Deserialize internal state from a previous version.
    fn deserialize(state: &[u8]) -> Result<Self, RuntimeError>
    where
        Self: Sized;

    /// Check if two actors have equivalent state (for testing).
    fn state_matches(&self, other: &Self) -> bool;
}

/// Result of a code change operation.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeChangeResult {
    /// New version loaded and made current for new processes.
    Upgraded {
        /// Module name that was upgraded.
        module: String,
        /// New version string.
        new_version: String,
        /// Old version that was replaced.
        old_version: String,
    },
    /// Code change failed - no processes affected.
    Failed {
        /// Reason for failure.
        reason: RuntimeError,
    },
}

#[cfg(test)]
mod tests;
