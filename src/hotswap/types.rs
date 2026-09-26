use crate::error::RuntimeError;
use std::any::Any;

/// Type alias for the message handler function pointer.
pub type MessageHandler = unsafe fn(
    *mut (),
    Box<dyn Any + Send>,
    &mut crate::actor::ActorContext,
) -> Result<(), crate::actor::ActorError>;

/// Type alias for the serialize function pointer.
pub type SerializeFn = unsafe fn(*mut ()) -> Vec<u8>;

/// Type alias for the deserialize function pointer.
pub type DeserializeFn = unsafe fn(Vec<u8>) -> Result<*mut (), RuntimeError>;

/// Type alias for the migrate function pointer.
pub type MigrateFn = unsafe fn(Vec<u8>, &str) -> Result<Vec<u8>, RuntimeError>;

/// Type alias for the drop function pointer.
pub type DropFn = unsafe fn(*mut ());

/// Type-erased vtable for a dynamically-loaded actor implementation.
///
/// This is the C-style interface that allows Runact to interact with actors
/// loaded from dynamic libraries without knowing their concrete types at
/// compile time. Each function pointer corresponds to a capability the runtime
/// needs: message handling, serialization, and construction.
pub struct ActorVTable {
    /// Semantic version of this module (e.g. "1.0.0")
    pub version: String,
    /// Module/type name this vtable belongs to
    pub type_name: String,
    /// Handle a message for this actor.
    pub handle_message_fn: MessageHandler,
    /// Serialize the actor's internal state to bytes.
    pub serialize_fn: SerializeFn,
    /// Deserialize state and create a new actor instance.
    pub deserialize_fn: DeserializeFn,
    /// Migrate state from one version to another.
    pub migrate_fn: MigrateFn,
    /// Clean up the actor's heap when it is terminated.
    pub drop_fn: DropFn,
}

/// Specification for spawning a dynamic actor with hot code loading.
#[derive(Clone)]
pub struct ActorSpec {
    /// Module name — must match what was registered with CodeManager
    pub module_name: String,
    /// Initial state bytes, or empty for new actor initialization
    pub initial_state: Vec<u8>,
}

/// A unit of work to be processed by a worker child actor (Pattern 2).
#[derive(Debug)]
pub struct WorkItem<T> {
    pub id: u64,
    pub payload: T,
}

/// Messages sent to the worker supervisor actor.
pub enum WorkerMessage<T, R> {
    /// Submit a work item — supervisor spawns a child worker actor (Pattern 2)
    SubmitWork {
        work: WorkItem<T>,
        responder: WorkerResponder<R>,
    },
    /// Internal: child worker completed and is terminating
    ChildCompleted {
        work_id: u64,
        result: Result<R, WorkerError>,
    },
    /// Graceful shutdown of the supervisor
    Shutdown,
}

/// Reply channel for work results.
pub struct WorkerResponder<R> {
    pub sender: crossbeam_channel::Sender<Result<R, WorkerError>>,
}

impl<R> WorkerResponder<R> {
    pub fn respond(self, result: Result<R, WorkerError>) {
        let _ = self.sender.send(result);
    }
}

/// Errors that can occur during work processing.
pub enum WorkerError {
    /// Worker actor panicked during processing
    Panic(String),
    /// Worker was cancelled (e.g. supervisor shutting down)
    Cancelled,
    /// Work processing exceeded time limits
    Timeout,
    /// State migration between versions failed
    MigrationFailed(String),
}

/// Handle returned when spawning a dynamic actor.
/// Like an ActorId but also tracks which version it runs.
pub struct VersionedActorRef {
    pub id: crate::actor::ActorId,
    pub version: String,
    pub module_name: String,
}
