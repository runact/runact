# Chapter 5: Actor Context

`ActorContext` is the actor's interface to the runtime. It is passed as
`&mut ActorContext` to every `handle` call. This chapter covers all the
methods you need to: send messages, reply to requests, schedule timers,
spawn async tasks, and offload compute work.

---

## 5.1 The Context API

```rust
pub struct ActorContext { /* private fields */ }

impl ActorContext {
    pub fn actor_id(&self) -> ActorId;

    pub fn send_to<M: Send + 'static>(
        &self, target: ActorId, message: M
    ) -> Result<(), RuntimeError>;

    pub fn reply<M: Send + 'static>(
        &self, message: M
    ) -> Result<(), RuntimeError>;

    pub fn is_request(&self) -> bool;

    pub fn spawn_compute<F, T>(
        &self, job: F
    ) -> Result<ComputeHandle<T>, ComputeError>
    where F: FnOnce() -> T + Send + 'static, T: Send + 'static;

    pub fn schedule_timer<M: Send + 'static>(
        &self, duration: Duration, message: M
    ) -> Result<TimerId, RuntimeError>;

    pub fn schedule_interval<M: Clone + Send + Sync + 'static>(
        &self, interval: Duration, message: M
    ) -> Result<TimerId, RuntimeError>;

    pub fn cancel_timer(&self, timer_id: TimerId);
}
```

## 5.2 Sending Messages to Other Actors

`ctx.send_to(target, message)` pushes a message onto another actor's
mailbox. This is how actors talk to each other.

```rust
struct Router {
    worker: ActorId,
}

impl Actor for Router {
    type Message = RouteMsg;

    fn handle(&mut self, msg: RouteMsg, ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        match msg {
            RouteMsg::Request(data) => {
                // Forward to the worker actor
                ctx.send_to(self.worker, WorkMsg::Do(data))
                    .map_err(|e| ActorError::Handler(e.to_string()))?;
            }
            // ...
        }
        Ok(())
    }
}
```

### send_to vs runtime.send

| Method | Called from | Blocks? | Use case |
|--------|-------------|---------|----------|
| `ctx.send_to` | Inside an actor handler | No (uses `try_send`) | Actor → actor messaging |
| `runtime.send` | Outside actors (main thread, external) | No (uses `try_send`) | External → actor messaging |
| `runtime.send_blocking` | External threads only | Yes (blocks until accepted) | High-priority external messages |

## 5.3 Reply-to-Sender (Request/Reply)

When an actor receives a message via `Runtime::request` (not `send`), the
runtime knows the caller expects a reply. `ctx.reply(message)` sends the
reply back.

```rust
// External code sends a request
let request_handle = runtime.request(counter_id, CounterMsg::Get).unwrap();

// Inside the Counter actor:
fn handle(&mut self, msg: Self::Message, ctx: &mut ActorContext)
    -> Result<(), ActorError>
{
    match msg {
        CounterMsg::Get => {
            // ctx.is_request() is true here
            ctx.reply(self.value)
                .map_err(|e| ActorError::Handler(e.to_string()))?;
        }
        // ...
    }
    Ok(())
}

// External code receives the reply
let value: u64 = request_handle.recv().unwrap();
```

If the message was sent fire-and-forget (via `send`), `ctx.is_request()`
returns `false` and `ctx.reply(...)` is a no-op.

## 5.4 Scheduling Timers

### One-shot Timer

```rust
ctx.schedule_timer(Duration::from_secs(5), WakeUp {})?;
```

After 5 seconds, the actor receives `WakeUp {}`. The actor continues
processing other messages during the wait — the timer is handled by the
timer service, not by blocking the actor.

### Periodic Timer (Interval)

```rust
ctx.schedule_interval(Duration::from_secs(10), Tick {})?;
```

The message must be `Clone + Send + Sync`. Each tick delivers a clone of
the message. Returns a `TimerId` so you can cancel it later:

```rust
let timer_id = ctx.schedule_interval(Duration::from_secs(10), Tick {})?;
// ...
ctx.cancel_timer(timer_id); // stops the interval
```

### Use Case: Heartbeat

```rust
struct Heartbeat {
    interval_id: Option<TimerId>,
}

impl Actor for Heartbeat {
    type Message = HeartMsg;

    fn handle(&mut self, msg: HeartMsg, ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        match msg {
            HeartMsg::Start => {
                self.interval_id = Some(
                    ctx.schedule_interval(Duration::from_secs(30), HeartMsg::Beat)?
                );
            }
            HeartMsg::Beat => {
                println!("Heartbeat at {:?}", std::time::Instant::now());
            }
            HeartMsg::Stop => {
                if let Some(id) = self.interval_id.take() {
                    ctx.cancel_timer(id);
                }
            }
        }
        Ok(())
    }
}
```

## 5.5 Offloading CPU-Bound Work

`ctx.spawn_compute(job)` runs a closure on the **compute pool** — a
separate thread pool for CPU-intensive work. The actor is not blocked.

```rust
struct DataProcessor;

impl Actor for DataProcessor {
    type Message = ProcessFile;

    fn handle(&mut self, msg: ProcessFile, ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        let handle = ctx.spawn_compute(move || {
            // This runs on a compute worker thread
            expensive_hash_computation(&msg.data)
        })?;

        // Store the handle; check it later when the result is ready
        // (e.g., in response to a Poll message)
        self.pending.insert(msg.file_id, handle);
        Ok(())
    }
}
```

The `ComputeHandle<T>` lets you check if the work is done (see Chapter 11).

## 5.6 The actor_id() Method

```rust
let my_id = ctx.actor_id();
ctx.send_to(other_actor, ForwardMsg { from: my_id })?;
```

This is essential for:
- Self-messaging (sending a message to yourself).
- Registering with a registry actor.
- Including your identity in forwarded messages.

## 5.7 Important: Handle Must Return Quickly

`ActorContext` methods are all **non-blocking**. `send_to` uses
`try_send` — if the target's mailbox is full, it returns
`RuntimeError::MailboxFull` immediately. `spawn_compute` submits work to a
bounded queue and returns immediately. `schedule_timer` just registers
with the timer service.

This is by design: actors are cooperative. An actor must process one
message, then yield back to the scheduler. Never block inside `handle`.
