# Chapter 3: Your First Actor

This chapter walks through a complete, runnable Runact program — from
`Cargo.toml` to shutdown — and explains each line.

---

## 3.1 Project Setup

```toml
# Cargo.toml
[package]
name = "runact-hello"
version = "0.1.0"
edition = "2024"

[dependencies]
runact = { path = "../runact" }
```

> **Note:** In this book, code examples use the local path to the runact
> crate. When publishing, use the crates.io version.

## 3.2 The Complete Example

```rust
use runact::{Actor, ActorContext, ActorError, Runtime};

/// A simple actor that prints a greeting.
struct Greeter;

impl Actor for Greeter {
    type Message = String;

    fn handle(
        &mut self,
        msg: String,
        _ctx: &mut ActorContext,
    ) -> Result<(), ActorError> {
        println!("Hello, {}!", msg);
        Ok(())
    }
}

fn main() {
    // 1. Create the runtime
    let mut runtime = Runtime::new().unwrap();

    // 2. Spawn the actor
    let id = runtime.spawn(Greeter).unwrap();

    // 3. Send a message
    runtime.send(id, "world".to_string()).unwrap();

    // 4. Shut down
    runtime.shutdown();
}
```

## 3.3 Line-by-Line Walkthrough

### Imports

```rust
use runact::{Actor, ActorContext, ActorError, Runtime};
```

Four imports cover the whole public API surface of the core types:

| Import          | Role                                  |
|-----------------|---------------------------------------|
| `Actor`         | The trait you implement               |
| `ActorContext`  | Methods for messaging, timers, compute|
| `ActorError`    | Return type for error handling        |
| `Runtime`       | Top-level coordinator                 |

### The Actor

```rust
struct Greeter;
```

An actor is just a normal Rust struct. You can add fields for state:

```rust
struct Counter {
    count: u64,
}
```

The struct must be `Send + 'static` (guaranteed by the `Actor` trait bound).
This means no `&T` references to non-`Send` data.

### Implementing Actor

```rust
impl Actor for Greeter {
    type Message = String;
```

The `type Message = String` declares what kind of message this actor accepts.
Any type works as long as it is `Send + 'static` — `String`, structs, enums,
tuples, or even `()`.

### The Handle Method

```rust
    fn handle(
        &mut self,
        msg: String,
        _ctx: &mut ActorContext,
    ) -> Result<(), ActorError> {
        println!("Hello, {}!", msg);
        Ok(())
    }
```

- `&mut self` — you own the actor's state for the duration of this call.
  Mutate fields freely.
- `msg: String` — the message, **moved** into the handler. No copying, no
  aliasing.
- `_ctx: &mut ActorContext` — use this to send messages to other actors,
  spawn timers, submit compute work, or spawn tasks. We prefix with `_`
  because this actor doesn't need it yet.
- `Result<(), ActorError>` — return `Ok(())` on success. Return
  `Err(ActorError::...)` if the message couldn't be processed; the
  supervisor decides what to do (typically restart the actor).

### Creating the Runtime

```rust
let mut runtime = Runtime::new().unwrap();
```

`Runtime::new()` sets up the scheduler, compute pool, timer service, and
task executor with default configuration. It returns `Result<Runtime,
RuntimeError>` — the `unwrap()` is fine for examples, but production code
should handle the error.

### Spawning

```rust
let id = runtime.spawn(Greeter).unwrap();
```

`spawn` takes ownership of the actor, assigns it an `ActorId`, and starts
polling it. The actor won't process messages until the scheduler gets to
it — but since we send a message right after spawning, the scheduler will
pick it up in the next cycle.

### Sending a Message

```rust
runtime.send(id, "world".to_string()).unwrap();
```

`send` is fire-and-forget. The message is pushed onto the actor's mailbox.
If the mailbox is full (bounded, default capacity), `send` returns
`RuntimeError::MailboxFull`. `send` takes `mut self` — it needs `&mut`
to the runtime because it interacts with the scheduler's registry.

### Shutting Down

```rust
runtime.shutdown();
```

`shutdown` signals all actors, async tasks, timers, and compute workers to
stop. It blocks until they finish or the shutdown timeout expires. After
`shutdown`, no new actors can be spawned and no new messages can be sent.

## 3.4 What Happens When You Run It

1. `Runtime::new()` starts worker threads (one per CPU core by default).
2. `spawn(Greeter)` creates the actor, assigns it `ActorId(1)`, and pushes
   it onto a worker's run queue.
3. `send(id, "world")` pushes the message onto the actor's mailbox. Since
   the actor now has a pending message, the scheduler ensures a worker
   will poll it.
4. The scheduler worker picks up the actor, calls `handle`, and prints
   `Hello, world!`.
5. `shutdown()` drains all pending work and joins worker threads.

## 3.5 Making It Do Work

The `Greeter` above is trivial — it prints and exits. Real actors use
`ActorContext` to interact with the runtime. Here is a slightly more
advanced example that uses a timer and sends to itself:

```rust
use runact::{Actor, ActorContext, ActorError, Runtime};
use std::time::Duration;

struct Ticker {
    remaining: u32,
}

impl Actor for Ticker {
    type Message = ();

    fn handle(
        &mut self,
        _msg: (),
        ctx: &mut ActorContext,
    ) -> Result<(), ActorError> {
        if self.remaining == 0 {
            println!("Done!");
            return Ok(());
        }

        println!("Tick... {} remaining", self.remaining);
        self.remaining -= 1;

        // Schedule a message to self after 1 second
        ctx.schedule_timer(Duration::from_secs(1), ())
            .map_err(|e| ActorError::Handler(e.to_string()))?;
        Ok(())
    }
}

fn main() {
    let mut runtime = Runtime::new().unwrap();
    let id = runtime.spawn(Ticker { remaining: 3 }).unwrap();
    runtime.send(id, ()).unwrap();
    runtime.shutdown();
}
```

This example uses:
- **State** (`remaining` field) that persists across messages.
- **`ctx.schedule_timer`** — sends the actor's own message type after a delay.
- **Self-messaging** — the actor sends itself `()` messages on a timer.

The output would be:

```
Tick... 3 remaining
Tick... 2 remaining     (1 second later)
Tick... 1 remaining     (1 second later)
Done!                    (1 second later)
```

Chapter 4 covers the `Actor` trait in full detail, and Chapter 5 covers
`ActorContext` — the methods you use to interact with the runtime from
inside an actor.
