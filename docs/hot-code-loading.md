# Hot Code Loading (Pattern 2: Dynamic Supervision)

## Overview

Runact supports **Pattern 2** hot code loading — the ability to load new versions of actor modules at runtime and have **newly spawned child actors automatically use the new code**, without restarting the runtime or affecting existing child actors.

This feature is gated behind the `hot_code_loading` Cargo feature and uses the `libloading` crate to dynamically load Rust dynamic libraries (`.so`/`.dylib`/`.dll`).

## Pattern 2: Dynamic Supervision

In Pattern 2, a supervisor receives work items and spawns a **new child actor per work unit** (rather than maintaining a fixed pool of long-lived children). This is what enables hot code loading: when a new version is loaded, `DynamicSupervisor::prepare_work_spawn()` always consults `CodeManager` for the current version, so new child actors spawned for subsequent work items automatically run the latest code.

```text
Pattern 2 (Dynamic Supervision):

DynamicSupervisor (work receiver)
   │
   ├── Work 1 → Child A (v1.0)
   ├── Work 2 → Child B (v1.0)
   │                                 ← upgrade happens
   ├── Work 3 → Child C (v1.1) ← new code automatically!
   └── Work 4 → Child D (v1.1)

Existing children (A, B) keep v1.0 until they complete.
New children (C, D) get v1.1 automatically.
```

## How It Works

### Core Concept: CodeManager + ActorFactory

```text
Runtime
├── CodeManager (version registry)
│   └── module_name → [CodePath v1.0, CodePath v1.1, ...]
│                      └─ current_version → always latest
├── ActorFactory (spawn_dynamic always uses current version)
│   └── prepare_dynamic_spawn() → get_current_vtable() → latest version
└── DynamicSupervisor (Pattern 2 actor)
    └── prepare_work_spawn() → factory.prepare_dynamic_spawn() → always current
```

### The Key Function

`ActorFactory::prepare_dynamic_spawn()` always calls `CodeManager::get_current_vtable()`, which returns the latest version's vtable. This is what makes "new processes use new code" work:

```rust
impl ActorFactory {
    pub fn prepare_dynamic_spawn(
        &self,
        spec: &ActorSpec,
    ) -> Result<Arc<ActorVTable>, RuntimeError> {
        let cm = self.code_manager.lock().unwrap();
        cm.get_current_vtable(&spec.module_name)  // ← always returns latest!
            .ok_or(RuntimeError::RuntimeStopped)
    }
}
```

## Usage

### 1. Implement HotSwappable Actors

Actors that support hot code loading implement the `HotSwappable` trait:

```rust
use runact::{Actor, ActorContext, ActorError, HotSwappable};
use runact::RuntimeError;

struct Worker {
    task_id: u64,
    config: String,
}

impl Actor for Worker {
    type Message = String;

    fn handle(&mut self, msg: String, ctx: &mut ActorContext) -> Result<(), ActorError> {
        // Process work items...
        Ok(())
    }
}

impl HotSwappable for Worker {
    const VERSION: &'static str = "1.0.0";

    fn serialize(&self) -> Vec<u8> {
        // Serialize state for migration
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.task_id.to_le_bytes());
        buf.extend_from_slice(self.config.as_bytes());
        buf
    }

    fn deserialize(state: &[u8]) -> Result<Self, RuntimeError>
    where
        Self: Sized,
    {
        // Deserialize from state bytes
        if state.len() < 8 {
            return Ok(Worker { task_id: 0, config: String::new() });
        }
        let task_id = u64::from_le_bytes(state[0..8].try_into().unwrap());
        let config = String::from_utf8(state[8..].to_vec()).unwrap_or_default();
        Ok(Worker { task_id, config })
    }

    fn state_matches(&self, other: &Self) -> bool {
        self.task_id == other.task_id && self.config == other.config
    }
}
```

### 2. Create a DynamicSupervisor

```rust
use runact::{Runtime, DynamicSupervisor};
use std::sync::Arc;

let runtime = Runtime::new()?;

// Load the initial version of the actor module
runtime.load_actor_module("Worker", "./worker_v1.so")?;

// Create a factory that always uses the current version
let factory = Arc::new(runtime.actor_factory().clone());
let supervisor = DynamicSupervisor::<String, String>::new("Worker", factory);

// The supervisor can be used standalone or spawned via Runtime::spawn()
let _ = supervisor; // Use prepare_work_spawn() directly
```

### 3. Load a New Version — New Processes Use New Code

```rust
// After existing children complete, load a new version:
let result = runtime.load_actor_module("Worker", "./worker_v2.so")?;
// → CodeChangeResult::Upgraded {
//     new_version: "2.0.0",
//     old_version: "1.0.0",
// }

// Now any NEW child spawned by the supervisor uses v2.0.0!
// Existing children from before the upgrade still use v1.0.0.
```

### 4. Prepare Work Spawns Using Current Version

```rust
// Each call to prepare_work_spawn uses the CURRENT version
let state_bytes = vec![1, 2, 3, 4]; // initial state for the child
let vtable = supervisor.prepare_work_spawn(state_bytes)?;

// After upgrade: vtable.version == "2.0.0"
// Before upgrade: vtable.version == "1.0.0"

// You can check the current version at any time:
assert_eq!(supervisor.current_version(), Some("2.0.0".to_string()));
```

### 5. Using the WorkerMessage Protocol

The `DynamicSupervisor` integrates with a `WorkerMessage` protocol:

```rust
use runact::{Actor, ActorContext, ActorError, WorkerMessage, WorkerResponder};

struct SupervisorActor {
    inner: DynamicSupervisor<WorkItem, WorkResult>,
}

impl Actor for SupervisorActor {
    type Message = WorkerMessage<WorkItem, WorkResult>;

    fn handle(&mut self, msg: Self::Message, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            WorkerMessage::SubmitWork { work, responder } => {
                // prepare_work_spawn always uses current version
                self.inner.prepare_work_spawn(work.payload)?;
                // ... spawn child, track completion ...
            }
            WorkerMessage::ChildCompleted { work_id, result } => {
                responder.respond(result);
            }
            WorkerMessage::Shutdown => ctx.stop(),
        }
        Ok(())
    }
}
```

## Migration Registry

When upgrading state from an old version, use the `MigrationRegistry`:

```rust
use runact::MigrationRegistry;

let mut registry = MigrationRegistry::new();

// Register a migration from v1.0 to v2.0
registry.register("Worker", "1.0.0", "2.0.0", |old_state: Vec<u8>| -> Vec<u8> {
    // Transform state from v1 format to v2 format
    let mut new_state = old_state;
    // ... migration logic ...
    new_state
});

// Chain migrations: v1.0 → v2.0 → v3.0
registry.register("Worker", "2.0.0", "3.0.0", |state: Vec<u8>| -> Vec<u8> {
    state
});

// Migrate state automatically through the chain
let migrated = registry.migrate("Worker", "1.0.0", "3.0.0", old_state)?;
```

## Architecture

### CodeManager

Tracks all loaded versions of modules. Each module has a list of `CodePath` entries, with a "current" index pointing to the latest version.

```rust
pub struct CodeManager {
    /// Module name → list of versions (latest at end)
    code_paths: HashMap<String, Vec<CodePath>>,
    /// Module name → index of current version
    current_versions: HashMap<String, usize>,
}
```

Key behavior:
- `load_new_version()` appends to the version list and updates `current_versions`
- `get_current_vtable()` always returns the latest version's vtable
- `get_version_vtable()` retrieves a specific old version for migration
- `current_version()` returns the version string of the current version
- `list_versions()` returns all version strings for a module

### ActorVTable

A C-style vtable that enables dynamic dispatch without knowing concrete types:

```rust
pub struct ActorVTable {
    pub version: String,
    pub type_name: String,
    pub handle_message_fn: MessageHandler,
    pub serialize_fn: SerializeFn,
    pub deserialize_fn: DeserializeFn,
    pub migrate_fn: MigrateFn,
    pub drop_fn: DropFn,
}
```

### DynamicSupervisor

The Pattern 2 supervisor. Uses `ActorFactory::prepare_dynamic_spawn()` to ensure new child actors always use the current version:

```rust
pub fn prepare_work_spawn(
    &self,
    initial_state: Vec<u8>,
) -> Result<Arc<ActorVTable>, RuntimeError> {
    let spec = ActorSpec {
        module_name: self.module_name.clone(),
        initial_state,
    };
    self.factory.prepare_dynamic_spawn(&spec)  // always current version
}
```

## Integration with Runtime

The `Runtime` exposes hot code loading methods when the feature is enabled:

| Method | Description |
|--------|-------------|
| `Runtime::load_actor_module(name, path)` | Load new version, make it current for new processes |
| `Runtime::module_version(name)` | Get current version string |
| `Runtime::module_versions(name)` | List all loaded versions |
| `Runtime::actor_factory()` | Get reference to the `ActorFactory` |

## Feature Flags

Add to your `Cargo.toml`:

```toml
[dependencies]
runact = { version = "1.2", features = ["hot_code_loading"] }
```

When the feature is disabled, the `hotswap` module is entirely excluded from the build, adding zero overhead.

## Public API Summary

| Type | Description |
|------|-------------|
| `Runtime::load_actor_module(name, path)` | Load new version, make it current |
| `Runtime::module_version(name)` | Get current version string |
| `Runtime::module_versions(name)` | List all loaded versions |
| `Runtime::actor_factory()` | Get reference to the `ActorFactory` |
| `ActorFactory::prepare_dynamic_spawn(spec)` | Spawn using current version |
| `ActorFactory::current_version(name)` | Check current version |
| `ActorFactory::runtime_sender()` | Get RuntimeSender for spawning |
| `CodeManager` | Version registry (tracks all versions) |
| `DynamicSupervisor` | Pattern 2 supervisor (spawns child per work item) |
| `HotSwappable` | Trait for version-aware actor state |
| `MigrationRegistry` | Chain state migrations between versions |
| `CodeChangeResult` | Result of a code load (Upgraded/Failed) |
| `DynamicLibrary` | `libloading` wrapper for `.so`/`.dylib`/`.dll` |
| `ActorVTable` | C-style vtable for dynamic dispatch |
| `ActorSpec` | Specification for spawning a dynamic actor |
| `WorkerMessage` | Supervisor message protocol |
| `WorkItem<T>` | A unit of work to process |
| `WorkerResponder<R>` | Reply channel for work results |
| `WorkerError` | Errors during work processing |
| `VersionedActorRef` | Handle with version metadata |
