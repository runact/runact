//! Hot code loading module.
//!
//! Provides Erlang/OTP-style hot code loading for runact actors, enabling
//! Pattern 2 (dynamic supervision): supervisors spawn child actors per work
//! item, and new children automatically use the latest code version after a
//! hot code upgrade.
//!
//! Enable with the `hot_code_loading` cargo feature.

pub(crate) mod codemgr;
pub(crate) mod dynamic_supervisor;
pub mod factory;
pub mod loader;
pub mod migration;
pub(crate) mod spawn_listener;
pub(crate) mod tests;
pub(crate) mod types;

pub use codemgr::{CodeManager, CodePath};
pub use dynamic_supervisor::DynamicSupervisor;
pub use factory::ActorFactory;
pub use loader::{DynamicLibrary, LibraryLoader};
pub use migration::{MigrationFn, MigrationRegistry, StateMigration};
pub use spawn_listener::SpawnListener;
pub use types::{ActorSpec, ActorVTable, WorkItem, WorkerMessage};

/// Result of a hot code loading operation.
#[derive(Debug, Clone)]
pub enum CodeChangeResult {
    /// Module was loaded for the first time.
    Loaded {
        /// Module name.
        module: String,
        /// Version that was loaded.
        new_version: String,
    },
    /// Module was upgraded to a new version.
    Upgraded {
        /// Module name.
        module: String,
        /// Version that was loaded (the new current version).
        new_version: String,
        /// Previous version that was current before this load.
        old_version: String,
    },
}

/// Trait for actors that support hot code loading.
pub trait HotSwappable {
    /// Get the current version of this actor's module.
    fn current_version(&self) -> String;
    /// Serialize the actor's internal state to bytes.
    fn serialize_state(&self) -> Vec<u8>;
    /// Deserialize state and return a new actor instance.
    fn deserialize_state(state: Vec<u8>) -> Result<Self, crate::error::RuntimeError>
    where
        Self: Sized;
}
