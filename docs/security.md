# Runact Security

## Overview

Security in Runact is based on capabilities. Every operation that could affect the system requires explicit permission. No implicit trust is granted to any component, including AI agents.

## Core Principles

1. **Least privilege** — Grant only what is needed
2. **Explicit permissions** — No implicit trust
3. **Audit everything** — Record all significant operations
4. **Fail secure** — Deny by default
5. **Defense in depth** — Multiple layers of protection

## Capability System

### ResourceHandle

A resource handle is a type that can be stored in the runtime's resource registry. Any type that implements `ResourceHandle` can be turned into a `Capability`.

```rust
pub trait ResourceHandle: Any + Send + Sync + fmt::Debug + 'static {
    fn resource_type(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}
```

### Capability

A capability is a typed, owner-tracked wrapper around a resource handle.

```rust
pub struct Capability<H: ResourceHandle> {
    owner: ActorId,
    handle: Arc<H>,
}
```

Example implementation:

```rust
#[derive(Debug)]
struct DatabaseConnection {
    url: String,
}

impl ResourceHandle for DatabaseConnection {
    fn resource_type(&self) -> &str {
        "DatabaseConnection"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Create a capability:
let cap = Capability::new(actor_id, DatabaseConnection {
    url: "postgres://localhost/mydb".to_string(),
});

// Access the handle:
cap.handle().resource_type();  // "DatabaseConnection"
cap.owner();  // ActorId of the owning actor
```

### Capability Properties

- **Type-safe** — `Capability<DatabaseConnection>` is distinct from `Capability<FileHandle>`
- **Owner-tracked** — Each capability knows which actor owns it
- **Shared via Arc** — Cloning is cheap (`Arc::clone`)
- **Send + Sync** — Safe to share between threads

### ResourceRegistry

A type-safe registry for storing capabilities by their handle type:

```rust
pub struct ResourceRegistry {
    resources: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}
```

```rust
let registry = ResourceRegistry::new();

// Register a capability:
registry.register(cap);

// Retrieve by type:
let retrieved: Option<Capability<DatabaseConnection>> = registry.get();

// Check existence:
assert!(registry.has::<DatabaseConnection>());

// Remove:
let removed: Option<Capability<DatabaseConnection>> = registry.remove();
```

The registry uses `TypeId` for type-erased lookup, ensuring type safety without runtime overhead for type mismatches.

## Capability Examples

### File Handle

```rust
#[derive(Debug)]
struct FileHandle {
    path: String,
    writable: bool,
}

impl ResourceHandle for FileHandle {
    fn resource_type(&self) -> &str { "FileHandle" }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
```

### Network Socket

```rust
#[derive(Debug)]
struct NetworkSocket {
    host: String,
    port: u16,
}

impl ResourceHandle for NetworkSocket {
    fn resource_type(&self) -> &str { "NetworkSocket" }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
```

## Ownership Model

Each capability tracks which actor owns it:

```rust
let cap = Capability::new(actor_id, my_resource);
assert_eq!(cap.owner(), actor_id);
```

This enables ownership-based access control — actors can only use capabilities they own or have been explicitly granted (by receiving a `Capability` message from another actor).

## Thread Safety

All resource types must be `Send + Sync`:

```rust
pub trait ResourceHandle: Any + Send + Sync + fmt::Debug + 'static
```

This ensures resources can be safely shared across the runtime's worker threads.

## Panic Safety

Compute tasks are isolated — panics are caught at the worker boundary and surfaced as `ComputeError::WorkerPanic`. This prevents a single faulty operation from crashing the runtime.

## Best Practices

1. **One capability per resource type** — The registry uses `TypeId` for lookup
2. **Keep handles small** — They're stored in a type-erased registry
3. **Use capabilities for access control** — Only actors with the capability can use the resource
4. **Clone to share** — `Arc` makes sharing cheap
5. **Design capabilities around the principle of least privilege** — Grant only what the actor needs
6. **Log capability usage** — Use `tracing` to record capability-related operations
