# Hot Code Loading (Pattern 2: Dynamic Supervision)

## Overview

Runact supports **Pattern 2** hot code loading — the ability to load new versions of actor modules at runtime and have **newly spawned child actors automatically use the latest code version** after a hot code upgrade.

This implements Erlang/OTP's "dynamic supervisor" behavior: a supervisor receives work items and spawns a child actor per work unit. New children always use the current module version, so after a hot code upgrade, the next work item automatically gets processed by the new code.

## Feature Flag

All hot code loading functionality is gated behind the `hot_code_loading` cargo feature:

```toml
[dependencies]
runact = { version = "1.3", features = ["hot_code_loading"] }
```

The `hot_code_loading` feature has one optional dependency:
- `libloading` — for loading dynamic libraries at runtime

When the feature is disabled, all hot code loading code is compiled out (zero overhead).

## Architecture

### Core Components

| Component | Module | Description |
|-----------|--------|-------------|
| `CodeManager` | `codemgr.rs` | Version registry — tracks multiple versions of each module, only one is "current" |
| `ActorFactory` | `factory.rs` | Spawns versioned actors using `prepare_dynamic_spawn()` → always gets current vtable |
| `DynamicLibrary` | `loader.rs` | libloading wrapper for loading `.so`/`.dylib`/`.dll` at runtime |
| `DynamicSupervisor` | `dynamic_supervisor.rs` | Pattern 2 supervisor — spawns a child actor per work item |
| `SpawnListener` | `spawn_listener.rs` | Background thread that processes spawn requests from supervisors |
| `HotSwappable` | `mod.rs` | Trait for actors that support state serialization/deserialization |
| `MigrationRegistry` | `migration.rs` | State migration between versions |
| `WorkPayload` | `dynamic_supervisor.rs` | Trait for serializing work items into child actor initial state |

### Automatic Spawning (Pattern 2)

The key mechanism is in `ActorFactory::prepare_dynamic_spawn()`:

```
DynamicSupervisor::handle(SubmitWork)
    -> ctx.spawn_child(module_name, initial_state)        // sends SpawnRequest via channel
    -> SpawnListener thread receives request
    -> CodeManager::get_current_vtable(module_name)       // ALWAYS returns latest version
    -> child actor spawned with current-version vtable
```

The `ActorContext::spawn_child()` method sends a `SpawnRequest` via a crossbeam channel to the `SpawnListener` background thread. The `SpawnListener` looks up the **current** version's vtable from `CodeManager` and spawns the child actor with it.

Since `CodeManager::get_current_vtable()` always returns the latest version, new child actors automatically use the latest code after a hot code upgrade — no explicit coordination needed.

### Key Types

#### `DynamicSupervisor<T, R>`

A supervisor actor generic over:
- `T: WorkPayload` — the work payload type (serialized to bytes for child state)
- `R: Send + 'static` — the result type returned when a child completes

The supervisor implements `Actor` with `WorkerMessage<T, R>` as its message type.

#### `WorkerMessage<T, R>`

```rust
pub enum WorkerMessage<T, R> {
    SubmitWork { work: WorkItem<T>, responder: WorkerResponder<R> },
    ChildCompleted { work_id: u64, result: Result<R, WorkerError> },
    Shutdown,
}
```

#### `WorkPayload`

```rust
pub trait WorkPayload: Send + 'static {
    fn to_bytes(&self) -> Vec<u8>;
}
```

The supervisor calls `to_bytes()` on the payload to produce the initial state for the child actor.

#### `HotSwappable`

```rust
pub trait HotSwappable {
    fn current_version(&self) -> String;
    fn serialize_state(&self) -> Vec<u8>;
    fn deserialize_state(state: Vec<u8>) -> Result<Self, RuntimeError> where Self: Sized;
}
```

## Usage

### Loading a new version

```rust
let mut runtime = Runtime::new().unwrap();
runtime.load_actor_module("MyActor", "/path/to/libmyactor_v2.so")?;
```

### Creating a DynamicSupervisor

```rust
use runact::{Actor, ActorContext, ActorError, DynamicSupervisor, Runtime, WorkPayload};

// Define a work payload that can be serialized
struct MyWork(String);
impl WorkPayload for MyWork {
    fn to_bytes(&self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }
}

// The supervisor is itself an actor
impl Actor for DynamicSupervisor<MyWork, String> {
    type Message = runact::WorkerMessage<MyWork, String>;
    // ... handle method
}

// Spawn the supervisor
let factory = runtime.actor_factory();
let supervisor = DynamicSupervisor::new("MyActor", factory.clone());
let sup_id = runtime.spawn(supervisor).unwrap();

// Submit work — a child is automatically spawned with the current version
runtime.send(sup_id, runact::WorkerMessage::SubmitWork {
    work: runact::WorkItem { id: 1, payload: MyWork("data".into()) },
    responder: ...,
});
```

## Code Upgrade Flow

1. **Load new code**: `Runtime::load_actor_module("MyActor", "libmyactor_v2.so")`
   - `CodeManager::load_new_version()` loads the library, reads the vtable, marks it as current
   - Old children continue running the old version
   - New spawn requests will use v2.0

2. **New work arrives**: Supervisor receives `SubmitWork`
   - Calls `ctx.spawn_child("MyActor", initial_state)`
   - `SpawnRequest` sent to `SpawnListener`
   - `SpawnListener` calls `CodeManager::get_current_vtable("MyActor")` → gets v2.0 vtable
   - Child actor spawned with v2.0 code

3. **Result**: New child processes use the latest code. No restart of the supervisor needed.
