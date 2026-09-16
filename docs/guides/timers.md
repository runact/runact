# Timers

## Overview

Timers deliver messages to actors on a schedule. Runact supports one-shot timers and periodic timers with drift correction.

## One-Shot Timer

Send a message after a delay:

```rust
use std::time::Duration;

// From outside an actor:
runtime.schedule_timer(
    Duration::from_secs(5),
    actor_id,
    "wake up".to_string(),
);

// From inside an actor:
ctx.schedule_timer(Duration::from_millis(500), "ping".to_string())?;
```

## Periodic Timer

Send a message repeatedly at intervals:

```rust
// From outside an actor:
runtime.schedule_interval(
    Duration::from_secs(1),
    actor_id,
    Tick,  // Must be Clone + Send + Sync
);

// From inside an actor:
ctx.schedule_interval(Duration::from_secs(1), Tick)?;
```

The message must implement `Clone + Send + Sync` because it is cloned for each tick.

## Drift Correction

Periodic timers use scheduled-at-based rescheduling to avoid drift:

```rust
// Instead of: scheduled_at = now + interval  (drifts over time)
// Runact uses: scheduled_at = previous_scheduled_at + interval  (no drift)
```

This means intervals stay accurate even if message handling takes time.

## Cancelling Timers

```rust
use runact::TimerId;

let timer_id = ctx.schedule_timer(Duration::from_secs(10), "delayed".to_string())?;

// Later, cancel it:
ctx.cancel_timer(timer_id);
```

Or from outside:

```rust
let timer_id = runtime.schedule_timer(Duration::from_secs(10), actor_id, "delayed".to_string());
runtime.cancel_timer(timer_id);
```

## Handling Timer Messages

Timer messages arrive as regular actor messages. Use an enum variant to distinguish them:

```rust
enum AppMsg {
    Tick,
    UserInput(String),
}

impl Actor for App {
    type Message = AppMsg;

    fn handle(&mut self, msg: AppMsg, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            AppMsg::Tick => {
                // Periodic work
                self.update();
            }
            AppMsg::UserInput(input) => {
                // Handle input
            }
        }
        Ok(())
    }
}
```

## TimerId

Each timer gets a unique `TimerId` that can be used for cancellation:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);
```

## Timer Implementation

Runact's timer service runs on a dedicated thread that polls scheduled timers every 1ms:

- One-shot timers fire once, then are removed
- Periodic timers fire at intervals, rescheduled based on previous scheduled time (drift correction)
- Timer messages are sent directly to the actor's mailbox via `MessageEnvelope::Message`
- Cancellation removes the timer entry from the timer list

## Best Practices

1. **Use periodic timers for polling** — Check for updates, refresh UI, heartbeat
2. **Use one-shot timers for delays** — Debouncing, timeouts, scheduled actions
3. **Always cancel timers you no longer need** — Prevents stale message delivery
4. **Clone messages must be cheap** — Use `Arc<str>` or small enums for periodic messages
5. **Use actor-context timers for actor-owned timers** — `ctx.schedule_timer` / `ctx.schedule_interval`
6. **Use runtime timers for external scheduling** — `runtime.schedule_timer` / `runtime.schedule_interval`
