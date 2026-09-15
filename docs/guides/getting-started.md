# Getting Started

## Overview

This guide walks through the core Runact workflow: spawning actors, sending messages, and handling replies.

## Spawning an Actor

```rust
use runact::{Actor, ActorId, ActorContext, ActorError, Runtime};

struct Greeter;

impl Actor for Greeter {
    type Message = String;

    fn handle(&mut self, msg: String, _ctx: &mut ActorContext) -> Result<(), ActorError> {
        println!("Hello, {}!", msg);
        Ok(())
    }
}

let mut runtime = Runtime::new().unwrap();
let id = runtime.spawn(Greeter).unwrap();
```

`spawn` returns an `ActorId` — a stable, unique identifier for the actor.

## Sending Messages

### Fire-and-Forget

```rust
runtime.send(id, "world".to_string()).unwrap();
```

`send` is non-blocking. It uses `try_send` internally and returns `Err(MailboxFull)` if the actor's mailbox is at capacity (default: 1000).

### Blocking Send (External Threads Only)

```rust
runtime.send_blocking(id, "world".to_string()).unwrap();
```

`send_blocking` blocks until the mailbox accepts the message. **Never call this from inside an actor** — it would block the scheduler worker thread.

## Request-Reply

```rust
let handle = runtime.request(id, "what is your name?".to_string()).unwrap();
let reply: String = handle.recv().unwrap();
```

The actor replies via `ctx.reply(...)`:

```rust
impl Actor for Greeter {
    type Message = String;

    fn handle(&mut self, msg: String, ctx: &mut ActorContext) -> Result<(), ActorError> {
        if ctx.is_request() {
            ctx.reply(format!("I am Greeter"))?;
        }
        Ok(())
    }
}
```

### RequestHandle Methods

| Method | Behavior |
|--------|----------|
| `recv()` | Block until reply arrives |
| `try_recv()` | Return immediately — `Some(result)` or `None` |
| `recv_timeout(dur)` | Block up to `dur`, then return error |

## Actor-to-Actor Messaging

Inside an actor, use `ctx.send_to(target, msg)`:

```rust
struct Forwarder {
    target: ActorId,
}

impl Actor for Forwarder {
    type Message = String;

    fn handle(&mut self, msg: String, ctx: &mut ActorContext) -> Result<(), ActorError> {
        ctx.send_to(self.target, msg)?;
        Ok(())
    }
}
```

## Shutdown

```rust
runtime.shutdown().unwrap();
```

Shutdown is also called automatically when `Runtime` is dropped.

## Runtime Configuration

```rust
use runact::{Runtime, RuntimeConfig, ComputeConfig};

let config = RuntimeConfig {
    compute: ComputeConfig {
        max_workers: 4,
        queue_capacity: 1024,
        task_timeout: None,
    },
    mailbox_capacity: 2000,
    shutdown_timeout: std::time::Duration::from_secs(10),
};

let mut runtime = Runtime::with_config(config).unwrap();
```

## Observability

```rust
let stats = runtime.stats();
println!("Actors: {}, Requests: {}", stats.actor_count, stats.request_count);
```

All actor lifecycle events are logged via `tracing`:

- `info` — actor spawned, actor stopped
- `debug` — message sent, request sent
- `trace` — per-message handling
- `warn` — actor restarted
- `error` — max restarts exceeded

## Complete Example

```rust
use runact::{Actor, ActorId, ActorContext, ActorError, Runtime};

struct Counter {
    value: i64,
}

enum CounterMsg {
    Increment,
    Decrement,
    GetValue,
}

impl Actor for Counter {
    type Message = CounterMsg;

    fn handle(&mut self, msg: CounterMsg, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            CounterMsg::Increment => self.value += 1,
            CounterMsg::Decrement => self.value -= 1,
            CounterMsg::GetValue => {
                ctx.reply(self.value)?;
            }
        }
        Ok(())
    }
}

fn main() {
    let mut runtime = Runtime::new().unwrap();
    let id = runtime.spawn(Counter { value: 0 }).unwrap();

    runtime.send(id, CounterMsg::Increment).unwrap();
    runtime.send(id, CounterMsg::Increment).unwrap();
    runtime.send(id, CounterMsg::Decrement).unwrap();

    let handle = runtime.request(id, CounterMsg::GetValue).unwrap();
    let value: i64 = handle.recv().unwrap();
    assert_eq!(value, 1);
}
```
