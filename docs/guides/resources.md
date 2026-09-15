# Resources

## Overview

Resources provide type-safe, capability-based access to external state. A `Capability` wraps a resource handle with its owning actor, and a `ResourceRegistry` stores capabilities by type.

## Defining a Resource

```rust
use runact::{ResourceHandle, Capability, ResourceRegistry};
use std::fmt;

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
```

## Creating a Capability

```rust
let cap = Capability::new(
    owner_actor_id,
    DatabaseConnection { url: "postgres://localhost/mydb".to_string() },
);

assert_eq!(cap.owner(), owner_actor_id);
assert_eq!(cap.handle().resource_type(), "DatabaseConnection");
```

## Storing in a Registry

```rust
let registry = ResourceRegistry::new();
registry.register(cap);

// Later, retrieve by type:
let retrieved: Option<Capability<DatabaseConnection>> = registry.get();
assert!(retrieved.is_some());

// Check existence:
assert!(registry.has::<DatabaseConnection>());

// Remove:
let removed = registry.remove::<DatabaseConnection>();
assert!(removed.is_some());
assert!(!registry.has::<DatabaseConnection>());
```

## Ownership Model

Each capability tracks which actor owns it:

```rust
let cap = Capability::new(actor_id, my_resource);
assert_eq!(cap.owner(), actor_id);
```

This enables ownership-based access control — actors can only use capabilities they own or have been granted.

## Shared Handles

Capabilities use `Arc<H>` internally, so cloning a capability shares the underlying handle:

```rust
let cap1 = Capability::new(owner, resource);
let cap2 = cap1.clone();

// Both share the same handle:
assert!(std::sync::Arc::ptr_eq(&cap1.into_handle(), &cap2.into_handle()));
```

## Thread Safety

All resource types must be `Send + Sync`:

```rust
pub trait ResourceHandle: Any + Send + Sync + fmt::Debug + 'static {
    fn resource_type(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}
```

## Best Practices

1. **One capability per resource type** — The registry uses `TypeId` for lookup
2. **Keep handles small** — They're stored in a type-erased registry
3. **Use capabilities for access control** — Only actors with the capability can use the resource
4. **Clone to share** — `Arc` makes sharing cheap
