# Actors

## Overview

Actors are the fundamental unit of computation in Runact. An actor owns its mutable state and communicates through messages.

## Core Principle

> **An actor owns its mutable state; communication transfers messages and ownership rather than shared mutable state.**

## Actor Trait

```rust
pub trait Actor: Send + 'static {
    type Message: Send + 'static;

    fn handle(
        &mut self,
        message: Self::Message,
        context: &mut ActorContext,
    ) -> Result<(), ActorError>;
}
```

### Requirements

- `Send` — Actor can be sent between threads
- `'static` — Actor owns all its data
- `Message: Send + 'static` — Messages can be sent between threads

### Why Synchronous?

Rust code cannot safely be arbitrarily preempted at any instruction boundary. Runact uses cooperative scheduling at safe execution boundaries.

## ActorId

Stable, unique identifier for an actor.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActorId(u64);
```

- Created at spawn time
- Never reused within a runtime instance
- Serializable
- Orderable (for debugging)

## ActorContext

Provided to actor handlers.

```rust
pub struct ActorContext {
    actor_id: ActorId,
    sender: MessageSender,
}
```

### Capabilities

- Send messages to other actors
- Access own ActorId
- (Future) Schedule timers
- (Future) Spawn child actors

## Actor Lifecycle

```text
spawned → running → stopping → stopped
    │                  │
    │                  └──→ terminated
    │
    └──→ failed → restarting → running
```

### States

- **spawned** — Actor created, not yet scheduled
- **running** — Actor is executing
- **stopping** — Graceful shutdown initiated
- **stopped** — Actor completed normally
- **terminated** — Actor was forcibly stopped
- **failed** — Actor crashed or returned error
- **restarting** — Supervisor is restarting the actor

## Ownership Model

### Actor Owns State

```rust
struct BufferActor {
    content: String,  // Owned by this actor
    lines: Vec<String>,  // Owned by this actor
}

impl Actor for BufferActor {
    type Message = BufferMessage;

    fn handle(&mut self, msg: BufferMessage, _ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            BufferMessage::Insert(text) => {
                self.content.push_str(&text);  // ✅ Exclusive access
            }
            BufferMessage::Delete(range) => {
                self.content.drain(range);  // ✅ Exclusive access
            }
        }
        Ok(())
    }
}
```

### Communication Transfers Ownership

```rust
// Sender: ownership moves into channel
actor.send(BufferMessage::Insert("hello".to_string()));

// Receiver: ownership moves out of channel
let msg = receiver.recv();  // msg owns the data
```

### No Shared Mutable State

```rust
// ❌ FORBIDDEN by design
let shared_state = Arc::Mutex::new(0);
let state1 = shared_state.clone();
let state2 = shared_state.clone();

// ✅ CORRECT
// Each actor owns its state
// Communication via messages only
```

## Message Design

### Good Messages

```rust
// Simple, clear, ownership-transferable
enum BufferMessage {
    Insert(String),
    Delete(Range),
    Save,
    GetSnapshot,
}

// Request-response pattern
enum Request {
    GetData,
    GetStatus,
}

enum Response {
    Data(String),
    Status(StatusInfo),
}
```

### Bad Messages

```rust
// ❌ Sharing references
enum BadMessage {
    GetData(&mut String),  // Can't send mutable references
    Process(&[u8]),  // Lifetime issues
}

// ❌ Cloning everything
enum BadMessage {
    GetData(String),  // Forces cloning
}
```

## Examples

### Simple Counter

```rust
struct CounterActor {
    count: u64,
}

enum CounterMessage {
    Increment,
    Decrement,
    GetValue,
}

impl Actor for CounterActor {
    type Message = CounterMessage;

    fn handle(&mut self, msg: CounterMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            CounterMessage::Increment => {
                self.count += 1;
            }
            CounterMessage::Decrement => {
                self.count = self.count.saturating_sub(1);
            }
            CounterMessage::GetValue => {
                // Send response back
                ctx.send(Box::new(self.count))?;
            }
        }
        Ok(())
    }
}
```

### Buffer Actor

```rust
struct BufferActor {
    content: String,
    modified: bool,
}

enum BufferMessage {
    Insert { position: usize, text: String },
    Delete { range: Range<usize> },
    Save,
    GetContent,
}

impl Actor for BufferActor {
    type Message = BufferMessage;

    fn handle(&mut self, msg: BufferMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            BufferMessage::Insert { position, text } => {
                self.content.insert_str(position, &text);
                self.modified = true;
            }
            BufferMessage::Delete { range } => {
                self.content.drain(range);
                self.modified = true;
            }
            BufferMessage::Save => {
                // Save to disk
                self.modified = false;
            }
            BufferMessage::GetContent => {
                ctx.send(Box::new(self.content.clone()))?;
            }
        }
        Ok(())
    }
}
```

## Best Practices

1. **Keep messages simple** — Easy to understand, easy to handle
2. **Transfer ownership** — Don't share references
3. **One message, one action** — Each message should do one thing
4. **Handle all cases** — Use exhaustive matching
5. **Don't block** — Process message and yield back
6. **Design for failure** — What happens if handler panics?
