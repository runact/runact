# Chapter 4: The Actor Trait

The `Actor` trait is the heart of Runact. It defines what an actor is and
how the runtime interacts with it. This chapter covers the full trait,
its design rationale, and common patterns.

---

## 4.1 The Trait

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

Three items:

### `Send + 'static` (trait bound)

The `Actor: Send + 'static` bound means every actor must be safe to move
to another thread (`Send`) and must not hold references to local data
(`'static`). This allows the scheduler to freely migrate actors between
worker threads.

**What this implies:**
- All fields must be `Send`. Common types like `String`, `Vec`, `HashMap`
  are `Send`. Types like `Rc<T>` and `RefCell<T>` are NOT `Send`.
- No `&'a T` references with lifetimes (unless `'static`).

### `type Message: Send + 'static`

The message type must also be `Send + 'static` so it can be safely
transferred between threads through the mailbox channel.

### `fn handle(&mut self, message, context) -> Result<(), ActorError>`

This is called for **every** message the actor receives. Key points:

- `&mut self` — you have exclusive access to your actor's state. Mutate
  it freely; no locks needed.
- `message` — **moved** into the handler. No cloning, no borrowing.
- `context` — your interface to the runtime (sending messages, spawning
  tasks, timers, etc.). See Chapter 5.
- Return `Ok(())` on success, `Err(ActorError::...)` on failure.

## 4.2 Returning Errors

The `ActorError` enum has three variants:

```rust
pub enum ActorError {
    Handler(String),   // General handler error
    Cancelled,         // Actor was cancelled
    Timeout,           // Operation timed out
    Panic(String),     // Actor panicked (set by runtime)
}
```

You construct `Handler(String)` for application-level errors:

```rust
fn handle(&mut self, msg: MyMessage, ctx: &mut ActorContext)
    -> Result<(), ActorError>
{
    if msg.value < 0 {
        return Err(ActorError::Handler(
            "value must be non-negative".to_string()
        ));
    }
    // process...
    Ok(())
}
```

When `handle` returns `Err`, the supervisor decides what to do. By default,
`OneForOne` strategy restarts the actor with exponential backoff. The
actor's `handle` is then called again for the next message in its mailbox.

> **Note:** `handle` does NOT receive an `init` or `started` callback in
> the current API. Actors are initialized when spawned — just set up state
> in the constructor (the struct itself). If you need startup logic, send
> the actor a `Start` message as its first message.

## 4.3 A Complete Example: A Counter Actor

```rust
use runact::{Actor, ActorContext, ActorError};
use std::collections::HashMap;

/// Messages the Counter actor can receive.
#[derive(Debug)]
enum CounterMsg {
    /// Increment by `n` and return the new value.
    Increment(u64),
    /// Decrement by `n` and return the new value.
    Decrement(u64),
    /// Get the current value.
    Get,
}

/// A stateful counter actor.
struct Counter {
    value: u64,
}

impl Counter {
    fn new() -> Self {
        Counter { value: 0 }
    }
}

impl Actor for Counter {
    type Message = CounterMsg;

    fn handle(
        &mut self,
        msg: Self::Message,
        _ctx: &mut ActorContext,
    ) -> Result<(), ActorError> {
        match msg {
            CounterMsg::Increment(n) => {
                self.value += n;
                println!("Counter is now {}", self.value);
            }
            CounterMsg::Decrement(n) => {
                self.value = self.value.saturating_sub(n);
                println!("Counter is now {}", self.value);
            }
            CounterMsg::Get => {
                println!("Counter value is {}", self.value);
            }
        }
        Ok(())
    }
}
```

Key patterns shown:
- **Enum messages** — A common Rust pattern for actors. Each variant is
  a different operation. The message enum itself carries the arguments.
- **State in the struct** — `value` is private to the actor. No other
  actor can touch it directly.
- **No blocking** — The handler runs to completion synchronously.

## 4.4 Stateful Actors

An actor's struct fields are its persistent state. Each time `handle` is
called, `&mut self` gives you access to that state. After `handle`
returns, the state remains as you left it — the actor hasn't been
destroyed.

```rust
struct SessionManager {
    sessions: HashMap<ActorId, Session>,
}

impl Actor for SessionManager {
    type Message = SessionEvent;

    fn handle(&mut self, msg: SessionEvent, ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        match msg {
            SessionEvent::Login { user_id, actor_id } => {
                self.sessions.insert(actor_id, Session::new(user_id));
            }
            SessionEvent::Logout { actor_id } => {
                self.sessions.remove(&actor_id);
            }
            SessionEvent::GetUserCount(reply) => {
                let count = self.sessions.len();
                reply(count).map_err(|e| ActorError::Handler(e.to_string()))?;
            }
        }
        Ok(())
    }
}
```

## 4.5 Multi-Message-Type Actors

Sometimes an actor needs to handle messages of different types. Rust's
trait system requires a single `Message` type, but you can use an enum:

```rust
enum WorkerMsg {
    Task(Task),
    Timeout(TimerId),
    Shutdown,
}

impl Actor for Worker {
    type Message = WorkerMsg;
    // ...
}
```

This is the standard approach. Avoid `Box<dyn Any>` — it defeats Rust's
type safety and is slower.

## 4.6 Actor Lifecycle

```
spawn(Actor) ──► [mailbox: empty] ──► message ──► handle() ──► Ok/Err
                                    ┌─────────────► (mailbox drains)
                                    └─► no more msgs ──► idle (parked)
```

When an actor has no messages, it is **parked** — it consumes no scheduler
resources. When a message arrives, the scheduler unparks one worker to
process it. This is how Runact achieves massive actor counts (thousands or
millions of idle actors) without performance cost.

When `handle` returns `Err(ActorError::Panic(..))`, the actor has panicked.
The supervisor will restart it (if configured). When `Err(ActorError::Cancelled)`
is returned, the actor is explicitly being shut down.
