//! Pattern 2: Dynamic supervision actor.
//!
//! A supervisor that receives work items and spawns a child actor per work
//! unit. New children always use the current version from CodeManager, so
//! after a hot code upgrade the next work item automatically gets the new code.

use crate::actor::{Actor, ActorContext, ActorError};
use crate::hotswap::ActorFactory;
use crate::hotswap::types::{WorkerMessage, WorkerResponder};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

/// Trait for payloads that can be serialized into child actor initial state.
pub trait WorkPayload: Send + 'static {
    /// Serialize the payload into initial state bytes for the child actor.
    fn to_bytes(&self) -> Vec<u8>;
}

/// Pattern 2 dynamic supervisor actor.
///
/// Receives work items and spawns a child actor per item. New children
/// use the current module version via ActorFactory, giving automatic
/// hot code loading for new processes.
///
/// This implements Erlang's "dynamic supervisor" behavior: each work item
/// gets its own freshly-spawned child process, and new children always run
/// the latest code version after a hot code upgrade.
///
/// ## Automatic Spawning
///
/// When a `SubmitWork` message is received, the supervisor calls
/// `ctx.spawn_child()` which sends a spawn request to the runtime via
/// a channel. The runtime processes this request using the current version
/// from `CodeManager`, ensuring new children automatically use the latest code.
pub struct DynamicSupervisor<T, R>
where
    T: WorkPayload,
    R: Send + 'static,
{
    /// Module name to spawn child actors for
    pub module_name: String,
    /// Factory for spawning versioned actors (holds reference to CodeManager)
    factory: std::sync::Arc<ActorFactory>,
    /// Child work item counter for IDs
    work_counter: AtomicU64,
    /// Track pending work items (work_id -> responder)
    pending: HashMap<u64, WorkerResponder<R>>,
    _phantom: PhantomData<T>,
}

impl<T, R> DynamicSupervisor<T, R>
where
    T: WorkPayload,
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

    /// Get the current code version for this module.
    pub fn current_version(&self) -> Option<String> {
        self.factory.current_version(&self.module_name)
    }
}

impl<T, R> Actor for DynamicSupervisor<T, R>
where
    T: WorkPayload,
    R: Send + 'static,
{
    type Message = WorkerMessage<T, R>;

    fn handle(&mut self, msg: Self::Message, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            WorkerMessage::SubmitWork { work, responder } => {
                let work_id = self.work_counter.fetch_add(1, Ordering::Relaxed);
                self.pending.insert(work_id, responder);

                // PATTERN 2: Automatically spawn a child actor using the
                // CURRENT version. The runtime will look up the current
                // version's vtable in CodeManager and spawn the child with it.
                // The payload is serialized into initial_state bytes so the
                // child can reconstruct its state.
                let initial_state = work.payload.to_bytes();
                ctx.spawn_child(&self.module_name, initial_state)
                    .map_err(|e| ActorError::Handler(e.to_string()))?;
            }
            WorkerMessage::ChildCompleted { work_id, result } => {
                if let Some(responder) = self.pending.remove(&work_id) {
                    responder.respond(result);
                }
            }
            WorkerMessage::Shutdown => {
                // Supervisor is shutting down; existing children will be
                // terminated by the runtime's graceful shutdown.
            }
        }
        Ok(())
    }
}
