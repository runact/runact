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

Rust code cannot safely be arbitrarily preempted at any instruction boundary. Runact uses cooperative scheduling at safe execution boundaries. The handler processes one message and returns; the scheduler decides whether to continue with the next message or yield.

## ActorId

Stable, unique identifier for an actor.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ActorId(u64);
```

- Created at spawn time
- Never reused within a runtime instance
- Serializable (via `serde`)
- Orderable (for debugging)

```rust
impl ActorId {
    pub fn new(id: u64) -> Self;
    pub fn as_u64(self) -> u64;
}
```

Display format: `Actor(42)`.

## ActorContext

Provided to actor handlers. Gives the actor access to:

```rust
pub struct ActorContext {
    actor_id: ActorId,
    _sender: crossbeam_channel::Sender<MessageEnvelope>,
    senders: Option<SenderMap>,
    reply_sender: Option<ReplySender>,
    compute_sender: Option<crossbeam_channel::Sender<Task>>,
    timer_handle: Option<TimerHandle>,
}
```

### Capabilities

- `actor_id()` — Get the actor's own ID
- `send_to(target, message)` — Send a fire-and-forget message to another actor
- `reply(message)` — Reply to the sender of a request (no-op for fire-and-forget)
- `is_request()` — Check if the current message was sent as a request
- `spawn_compute(job)` — Submit a CPU-intensive task to the compute pool
- `schedule_timer(duration, message)` — Schedule a one-shot timer
- `schedule_interval(interval, message)` — Schedule a periodic timer
- `cancel_timer(timer_id)` — Cancel a previously scheduled timer

### External vs Internal Messaging

- **Actor-to-actor**: Use `ctx.send_to(target, msg)` (non-blocking, bounded mailbox)
- **External-to-actor**: Use `runtime.send(actor_id, msg)` (non-blocking) or `runtime.send_blocking(actor_id, msg)` (blocking)

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

The runtime logs lifecycle events via `tracing`:

- `info` — actor spawned, actor stopped
- `warn` — actor restarted (via supervisor)
- `error` — max restarts exceeded

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
runtime.send(actor_id, BufferMessage::Insert("hello".to_string())).unwrap();

// Actor-to-actor: ownership moves via message
ctx.send_to(target, BufferMessage::Insert("hello".to_string())).unwrap();
```

### No Shared Mutable State (as the default model)

```rust
// ❌ FORBIDDEN as the fundamental model
let shared_state = Arc::Mutex::new(0);
let state1 = shared_state.clone();
let state2 = shared_state.clone();

// ✅ CORRECT
// Each actor owns its state
// Communication via messages only
```

Shared state (`Arc<Mutex<T>>`, `RwLock<T>`, `Atomic<T>`) remains possible when genuinely required, but it is not the fundamental programming model.

## Request/Reply

Actors can send requests and receive replies via `Runtime::request`:

```rust
let handle = runtime.request(actor_id, GetValue)?;
let reply: Box<dyn Any + Send> = handle.recv()?;
```

Inside an actor, reply using `ctx.reply`:

```rust
impl Actor for Counter {
    type Message = CounterMessage;

    fn handle(&mut self, msg: CounterMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            CounterMessage::GetValue => {
                ctx.reply(self.count)?;
            }
        }
        Ok(())
    }
}
```

### RequestHandle Methods

| Method | Behavior |
|--------|----------|
| `recv()` | Block until reply arrives |
| `try_recv()` | Return immediately — `Ok` or `Err` |
| `recv_timeout(dur)` | Block up to `dur`, then return error |

The reply channel is bounded (capacity 1) to prevent uncontrolled memory growth.

## Message Design

### Good Messages

```rust
// Simple, clear, ownership-transferable
enum BufferMessage {
    Insert(String),
    Delete(Range<usize>),
    Save,
    GetSnapshot,
}
```

### Bad Messages

```rust
// ❌ Sharing references
enum BadMessage {
    GetData(&mut String),  // Can't send mutable references
    Process(&[u8]),  // Lifetime issues
}

// ❌ Requiring shared mutable state
enum BadMessage {
    GetData(Arc<Mutex<Data>>),  // Defeats the actor model
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
                ctx.reply(self.count)?;
            }
        }
        Ok(())
    }
}
```

### Forwarder Actor (Actor-to-Actor Messaging)

```rust
struct ForwardActor {
    target: Option<ActorId>,
}

enum ForwardMessage {
    SetTarget(ActorId),
    Forward(String),
}

impl Actor for ForwardActor {
    type Message = ForwardMessage;

    fn handle(&mut self, msg: ForwardMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            ForwardMessage::SetTarget(id) => {
                self.target = Some(id);
            }
            ForwardMessage::Forward(text) => {
                if let Some(target) = self.target {
                    ctx.send_to(target, LoggerMessage::Log(text))?;
                }
            }
        }
        Ok(())
    }
}
```

### Compute Actor (Offloading CPU Work)

```rust
struct DataProcessor {
    pending: Option<ComputeHandle<Vec<u8>>>,
}

enum DataMsg {
    Process(Vec<u8>),
    CheckResult,
    GetResult,
}

impl Actor for DataProcessor {
    type Message = DataMsg;

    fn handle(&mut self, msg: DataMsg, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            DataMsg::Process(data) => {
                let handle = ctx.spawn_compute(move || {
                    // CPU-intensive work on compute pool
                    data.iter().map(|b| b.wrapping_add(1)).collect()
                })?;
                self.pending = Some(handle);
            }
            DataMsg::CheckResult => {
                if let Some(ref handle) = self.pending {
                    if let Some(result) = handle.try_recv() {
                        match result {
                            Ok(value) => {
                                ctx.reply(value)?;
                                self.pending = None;
                            }
                            Err(e) => {
                                ctx.reply(ComputeError::WorkerPanic(e.to_string()))?;
                            }
                        }
                    }
                }
            }
            DataMsg::GetResult => {
                if let Some(ref handle) = self.pending {
                    match handle.recv_timeout(Duration::from_secs(1)) {
                        Ok(value) => {
                            ctx.reply(value)?;
                            self.pending = None;
                        }
                        Err(e) => {
                            ctx.reply(e)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
```

## Best Practices

1. **Keep messages simple** — Easy to understand, easy to handle
2. **Transfer ownership** — Don't share references in messages
3. **One message, one action** — Each message should do one thing
4. **Handle all cases** — Use exhaustive matching
5. **Don't block** — Process message and yield back to the scheduler
6. **Use compute pool for CPU work** — Never do long-running computation in an actor handler
7. **Design for failure** — What happens if handler panics? (Compute tasks are isolated; actor panics are surfaced to supervisors.)
8. **Use timers for delays** — Don't `thread::sleep` inside actors
9. **Reply to requests** — If `ctx.is_request()`, consider replying
10. **Cancel timers you no longer need** — Prevents stale message delivery
