# Chapter 10: Timers

Timers let actors schedule messages to arrive after a delay. This chapter
covers one-shot timers, periodic intervals, and the `TimerId` for
cancellation.

---

## 10.1 One-shot Timer

`ctx.schedule_timer(duration, message)` sends a message to the actor after
the specified delay:

```rust
use runact::{Actor, ActorContext, ActorError};
use std::time::Duration;

struct Delay;

impl Actor for Delay {
    type Message = ();

    fn handle(&mut self, _msg: (), ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        println!("Starting...");

        // Send myself () after 2 seconds
        ctx.schedule_timer(Duration::from_secs(2), ())
            .map_err(|e| ActorError::Handler(e.to_string()))?;

        println!("Timer scheduled — I am not blocked!");
        Ok(())
    }
}
```

Key properties:
- **Non-blocking**: `schedule_timer` returns immediately. The actor is
  free to process other messages while the timer runs.
- **The timer fires once**: After 2 seconds, the actor receives `()`.
- **The message type** must be `Send + 'static` (same as actor messages).
  It does NOT need to be the actor's `Message` type — Runact can route
  any `Send + 'static` type through the timer service.

## 10.2 Periodic Timer (Interval)

`ctx.schedule_interval(interval, message)` sends a message repeatedly at
fixed intervals:

```rust
struct Heartbeat;

impl Actor for Heartbeat {
    type Message = Beat;

    fn handle(&mut self, msg: Beat, ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        match msg {
            Beat::Start => {
                println!("Heartbeat started");
                ctx.schedule_interval(Duration::from_secs(30), Beat::Pulse)
                    .map_err(|e| ActorError::Handler(e.to_string()))?;
            }
            Beat::Pulse => {
                println!("Heartbeat at {:?}", std::time::Instant::now());
                // Pulse arrives every 30 seconds automatically
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
enum Beat { Start, Pulse }
```

### Message Requirements

The interval message must be `Clone + Send + Sync` because it is cloned
for each tick. `String`, `u32`, `Vec<u8>` all satisfy this.

### Returns TimerId

```rust
let timer_id = ctx.schedule_interval(Duration::from_secs(30), Beat::Pulse)?;
```

## 10.3 TimerId and Cancellation

Both `schedule_timer` and `schedule_interval` return a `TimerId`. Store
it to cancel the timer later:

```rust
struct Heartbeat {
    interval_id: Option<TimerId>,
}

impl Actor for Heartbeat {
    type Message = BeatMsg;

    fn handle(&mut self, msg: BeatMsg, ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        match msg {
            BeatMsg::Start => {
                self.interval_id = Some(
                    ctx.schedule_interval(Duration::from_secs(30), BeatMsg::Pulse)?
                );
            }
            BeatMsg::Pulse => {
                println!("Pulse");
            }
            BeatMsg::Stop => {
                if let Some(id) = self.interval_id.take() {
                    ctx.cancel_timer(id);
                }
            }
        }
        Ok(())
    }
}
```

## 10.4 Drift Correction

For periodic timers, Runact applies **drift correction**: each tick is
scheduled relative to the previous tick's target time, not the actual
fire time. This means if a tick fires late (e.g., the actor was busy),
subsequent ticks compensate to maintain the overall interval.

## 10.5 Runtime-Level Timers

The same timer service is accessible from the `Runtime` directly, for
scheduling messages to actors from outside:

```rust
let timer_id = runtime.schedule_timer(
    Duration::from_secs(10),
    actor_id,
    MyMessage::Timeout,
);
// ...
runtime.cancel_timer(timer_id);
```

This is useful when setting up timers before an actor is spawned, or
from external coordination code.

## 10.6 Use Cases

| Pattern | Timer type | Example |
|---------|-----------|---------|
| Delayed response | one-shot | Retry fetch after backoff |
| Heartbeat | interval | Send ping every 30s |
| TTL | one-shot | Expire a session after 1h |
| Polling | interval | Check health every 5s |
| Circuit breaker | one-shot | Open breaker for 60s after failure |
