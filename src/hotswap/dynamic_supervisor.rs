//! Pattern 2: Dynamic supervision actor.
//!
//! A supervisor that receives work items and spawns a child actor per work
//! unit. New children always use the current version from CodeManager, so
//! after a hot code upgrade the next work item automatically gets the new code.

use crate::hotswap::ActorFactory;
use crate::hotswap::types::{WorkerError, WorkerMessage, WorkerResponder};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

/// Pattern 2 dynamic supervisor actor.
///
/// Receives work items and spawns a child actor per item. New children
/// use the current module version via ActorFactory, giving automatic
/// hot code loading for new processes.
///
/// This implements Erlang's "dynamic supervisor" behavior: each work item
/// gets its own freshly-spawned child process, and new children always run
/// the latest code version after a hot code upgrade.
pub struct DynamicSupervisor<T, R>
where
    T: Send + 'static,
    R: Send + 'static,
{
    /// Module name to spawn child actors for
    pub module_name: String,
    /// Factory for spawning versioned actors (holds reference to CodeManager)
    factory: std::sync::Arc<ActorFactory>,
    /// Child work item counter for IDs
    work_counter: AtomicU64,
    /// Track pending work items (work_id -> responder) - reserved for future
    #[allow(dead_code)]
    pending: HashMap<u64, WorkerResponder<R>>,
    /// Phantom data for generic types
    _phantom: PhantomData<(T, R)>,
}

impl<T, R> DynamicSupervisor<T, R>
where
    T: Send + 'static,
    R: Send + 'static,
{
    pub fn new(module_name: &str, factory: std::sync::Arc<ActorFactory>) -> Self {
        Self {
            module_name: module_name.to_string(),
            factory,
            work_counter: AtomicU64::new(0),
            pending: HashMap::new(),
            _phantom: PhantomData,
        }
    }

    /// Prepare to spawn a child for a work item using the CURRENT version.
    ///
    /// This is the key Pattern 2 method: it calls `factory.prepare_dynamic_spawn()`
    /// which always uses the current version's vtable. After a hot code
    /// upgrade, this method returns the new version's vtable automatically.
    pub fn prepare_work_spawn(
        &self,
        initial_state: Vec<u8>,
    ) -> Result<std::sync::Arc<crate::hotswap::types::ActorVTable>, crate::error::RuntimeError>
    {
        let work_id = self.work_counter.fetch_add(1, Ordering::Relaxed);
        let _ = work_id; // Reserved for tracing/pending tracking

        let spec = crate::hotswap::types::ActorSpec {
            module_name: self.module_name.clone(),
            initial_state,
        };

        self.factory.prepare_dynamic_spawn(&spec)
    }

    /// Get the current code version for this module.
    pub fn current_version(&self) -> Option<String> {
        self.factory.current_version(&self.module_name)
    }

    /// Get the runtime sender for spawning child actors.
    pub fn runtime_sender(&self) -> &crate::runtime::RuntimeSender {
        self.factory.runtime_sender()
    }
}

// Silence unused: WorkerMessage is the message protocol for this pattern
#[allow(dead_code)]
type _Silence = WorkerMessage<(), ()>;

// Silence unused: WorkerError is part of the message protocol
#[allow(dead_code)]
type _Silence2 = WorkerError;
