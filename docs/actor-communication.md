# Runact Actor Communication Principles

This document defines the communication model for Runact actors. It is the reference for how actors interact, how messages flow, and what constraints the runtime enforces.

## 1. Actors Never Share Mutable State

An actor owns its state.

```
Actor A                    Actor B
┌───────────┐              ┌───────────┐
│ State A   │              │ State B   │
└───────────┘              └───────────┘
      │                          │
      └────── messages ──────────┘
```

No actor should directly mutate another actor's state.

Rust's ownership system enforces this wherever possible. Shared state (`Arc<Mutex<T>>`) remains possible when genuinely required, but it is not the fundamental programming model.

See also: [Actors](#621-actor-state-ownership)

---

## 2. Message Sending is Asynchronous

The fundamental operation is:

```rust
runtime.send(actor_id, message)?;
```

Or from inside an actor:

```rust
ctx.send_to(target, message)?;
```

It means:

> "Put this message into the recipient's mailbox and return."

It does **not** mean:

> "Wait for the recipient to process this message."

Therefore:

```rust
runtime.send(actor_id, message).unwrap();
```

should return immediately (unless the mailbox is full, in which case it returns `RuntimeError::MailboxFull`).

---

## 3. `send()` Must Not Wait for the Actor

This is an important distinction.

**Bad:**

```rust
runtime.send(id, msg).unwrap(); // would block until A processes msg
```

**Correct:**

```rust
runtime.send(id, msg).unwrap(); // enqueue and return
```

The sender should not depend on the recipient's execution speed.

---

## 4. Request/Reply is Still Asynchronous

Sometimes an actor needs an answer.

Don't turn that into synchronous actor blocking.

Instead:

```rust
let handle = runtime.request(actor_id, message)?;
```

Conceptually:

```
A
│
│ Request
▼
B
│
│ Reply
▼
A
```

The request returns a `RequestHandle`, not the result itself.

```rust
let handle = runtime.request(actor_id, message)?;
let reply = handle.recv_timeout(Duration::from_secs(1))?;
```

Or `try_recv` for non-blocking polling:

```rust
match handle.try_recv() {
    Ok(reply) => { /* handle reply */ }
    Err(_) => { /* still pending or failed */ }
}
```

But the crucial question is:

> What happens to the actor while it waits?

Inside an actor, the reply is received asynchronously — the actor handler returns and the result is delivered as a subsequent message. The actor must not call `recv()` (which blocks) inside its handler; it should submit work and poll later via `try_recv`.

For external threads (outside actors), `recv()` is acceptable since those threads are not scheduler workers.

---

## 5. An Actor Must Never Block Its Scheduler Worker

This is probably the most important runtime rule.

**Bad:**

```text
Worker 1
   │
   └── Actor A
          │
          └── waiting for B
```

Worker 1 should not become:

```text
Worker 1
   │
   └── BLOCKED
```

**Correct:**

```text
Actor A
   │
   └── waiting for reply
          │
          ▼
      scheduler
          │
          ├── Actor B
          ├── Actor C
          └── Actor D
```

A waiting actor should return control to the scheduler (via its handler returning), while the worker executes something else.

In Runact's current implementation, actors do not have an explicit "waiting" state for request/reply — instead, actors submit compute tasks and poll with `try_recv`, or use timers for timeouts. The handler always returns quickly.

---

## 6. Waiting is a State, Not Blocking

Think of an actor state machine:

```text
              ┌───────────┐
              │   Ready   │
              └─────┬─────┘
                    │
                    ▼
              ┌───────────┐
              │ Running   │
              └─────┬─────┘
                    │
          ┌─────────┼─────────┐
          │         │         │
          ▼         ▼         ▼
       Ready     Waiting   Sleeping
                    │
                    │ reply/timer/compute result
                    ▼
                  Ready
```

The actor can wait for:

- compute result (poll with `try_recv`)
- timer (timer message arrives later)
- external event (via messages)

without blocking an OS thread.

---

## 7. Actor-to-Actor Calls Should Not Hold Locks

**Avoid:**

```text
A
│
├── lock state
│
└── call B
      │
      └── B calls A
```

This creates traditional lock-style deadlocks.

Instead, actor state should normally be accessed only while that actor is executing.

This gives us:

- No locks across actor boundaries.
- Locks can still exist internally when genuinely necessary, but they should not be the normal communication mechanism.

---

## 8. Request/Reply Must Have Correlation IDs

A request carries an ID.

**Request:**

```text
Request
├── request id
├── payload
└── reply channel
```

**Reply:**

```text
Reply
├── request id
├── sender
└── result
```

In Runact, `Runtime::request` assigns a unique `request_id` (via an `AtomicU64` counter) and creates a bounded oneshot channel for the reply:

```rust
let request_id = self.request_counter.fetch_add(1, Ordering::Relaxed);
let (reply_tx, reply_rx) = crossbeam_channel::bounded(1);
```

The reply is sent back via `ctx.reply(message)`, which writes to the reply channel. The `RequestHandle` wraps the receiver.

The runtime can match replies correctly.

This becomes important when an actor has many outstanding requests.

---

## 9. Timeouts are Optional, Not Implicit

A request may specify a timeout:

```rust
let handle = runtime.request(actor_id, message)?;
let reply = handle.recv_timeout(Duration::from_secs(5))?;
```

If the timeout expires:

```text
Request
   │
   ├── Reply → success
   │
   └── Timeout → returns RecvTimeoutError::Timeout
```

But Runact should not automatically impose arbitrary timeouts.

A local computation might legitimately take 30 seconds.

Timeouts are application-level policy.

For compute tasks, `ComputeHandle::recv_timeout` provides the same pattern:

```rust
let handle = ctx.spawn_compute(|| expensive_work())?;
match handle.recv_timeout(Duration::from_secs(5)) {
    Ok(value) => { /* use value */ }
    Err(ComputeError::WorkerPanic("Timeout")) => { /* timed out */ }
    Err(e) => { /* other error */ }
}
```

---

## 10. Cancellation Should Be Cooperative

Never try to forcibly kill arbitrary Rust code.

**Bad model:**

```text
cancel()
   ↓
kill thread
```

**Instead:**

```text
cancel()
   ↓
CancellationToken
   ↓
computation checks token
   ↓
stops safely
```

For compute tasks:

```rust
let handle = ctx.spawn_compute(|| {
    // long-running work
})?;

// Later, cancel it:
handle.cancel();
```

The compute worker checks the cancellation flag before and after execution. If set before, the task is skipped (`ComputeResult::Cancelled`). If set after, the result is discarded.

This matters particularly for the Runact compute pool.

---

## 11. Fire-and-Forget is a First-Class Pattern

Sometimes no response is required.

```rust
runtime.send(actor_id, LogMessage::Info("hello".to_string()))?;
```

The sender doesn't care about a reply.

This should be extremely cheap.

```rust
ctx.send_to(target, Message::Notify)?;
```

---

## 12. Request/Reply is Not the Default

This distinction is important.

**Prefer:**

- event
- notification
- command
- message

**Over:**

- call
- wait
- reply

when possible.

**Example:**

```text
FileActor
   │
   └── FileChanged
           │
           ├── BufferActor
           ├── UIActor
           └── SearchActor
```

rather than having everyone synchronously query the file actor.

This encourages loose coupling. Fire-and-forget message passing is the primary communication mechanism.

---

## 13. Don't Make Every Operation Request/Reply

**Bad architecture:**

```text
UI
 ↓
Buffer.call()
 ↓
File.call()
 ↓
Git.call()
 ↓
Network.call()
 ↓
AI.call()
```

You end up recreating RPC inside one process.

**Better:**

```text
             ┌── BufferActor
             │
UIActor ──────┼── FileActor
             │
             ├── GitActor
             │
             ├── AIActor
             │
             └── LSPActor
```

Communication remains message-oriented.

---

## 14. Mailbox Backpressure Must Be Explicit

Asynchronous doesn't mean unlimited queues.

Consider:

```text
Producer
   │
   │ 1,000,000 messages/sec
   ▼
Mailbox
   │
   │ 100 messages/sec
   ▼
Actor
```

Eventually memory explodes.

Therefore Runact uses bounded mailboxes. The default capacity is 1000 messages, configurable via `RuntimeConfig.mailbox_capacity`.

**Policy:**

| Strategy | Behavior |
|----------|----------|
| `Reject` | Return `RuntimeError::MailboxFull` to sender |
| `Block` | Sender waits until space available (`send_blocking`) |

For `Runtime::send`:

> Use bounded mailboxes and return an explicit error when capacity is exhausted.

```rust
runtime.send(actor_id, message)?;
// Returns Err(RuntimeError::MailboxFull(actor_id)) if full
```

For external threads that can afford to block:

```rust
runtime.send_blocking(actor_id, message)?;
// Blocks until the message is accepted
```

---

## 15. External Threads Are Different

There is an important distinction between:

- Actor → Actor (via `ctx.send_to`)
- External OS thread → Actor (via `runtime.send` / `runtime.send_blocking`)
- External OS thread → Actor with reply (via `runtime.request`)

A normal OS thread can potentially block.

For example:

```rust
// From an external thread — acceptable
let handle = runtime.request(actor_id, msg)?;
let reply: String = handle.recv()?;
```

But inside an actor handler:

```rust
// Inside an actor handler — must NOT block the scheduler worker
// Instead, submit compute and poll with try_recv
let handle = ctx.spawn_compute(|| heavy_work())?;
// Return from handler, poll later
```

So Runact has two APIs:

```rust
// Actor context — non-blocking
ctx.send_to(target, msg)?;

// External/blocking context — may block
runtime.send_blocking(actor_id, msg)?;
runtime.request(actor_id, msg)?;
let handle = runtime.request(actor_id, msg)?;
handle.recv_timeout(Duration::from_secs(5))?;
```

This distinction makes the semantics clear.

---

## 16. Actor Code Should Be Mostly Deterministic

An actor should conceptually behave like:

```text
State + Message → New State + Effects
```

```rust
fn handle(&mut self, msg: Message, ctx: &mut ActorContext) -> Result<(), ActorError> {
    // State transition
    self.count += 1;
    // Effect
    ctx.reply(self.count)?;
    Ok(())
}
```

The actor owns the state transition.

External effects should go through runtime services (compute pool, timers, message sending).

This makes actors easier to test.

---

## 17. Effects Should Be Explicit

An actor shouldn't directly do everything.

**Good:**

```text
BufferActor
   │
   ├── state mutation
   │
   └── request FileActor to save
```

**Bad:**

```text
BufferActor
   │
   └── directly manipulates filesystem
```

This produces clearer architecture. Actors communicate intent through messages; side effects are handled by appropriate actors or the compute pool.

---

## 18. Long Computations Are Not Actor Work

Suppose an actor receives:

```text
CompileProject
```

It shouldn't perform a 30-second compilation inside its actor execution.

Instead:

```text
CompileActor
     │
     └── submit
          ↓
     Compute Pool
          │
          ↓
       Result
          │
          ↓
     CompileActor
```

This preserves responsiveness. Use `ctx.spawn_compute` to offload CPU-intensive work.

---

## 19. Failure is a Message/Runtime Event

An actor failing should not mean:

```text
panic
 ↓
Runact dies
```

**Instead:**

```text
Compute Task
  │
  │ panic
  ▼
Runtime catches failure (catch_unwind in compute worker)
  │
  └── ComputeResult::Panic(msg)
       │
       ▼
      Actor (receives via ComputeHandle)
```

Compute tasks are isolated — panics are caught at the worker boundary and surfaced as `ComputeError::WorkerPanic(msg)`. The actor and worker thread survive.

For actor handler panics (not currently caught in v1.0.0 but planned), the model is:

```text
Actor
  │
  │ panic
  ▼
Runtime catches failure
  │
  ├── record failure
  ├── notify supervisor
  ├── clean actor resources
  └── apply restart strategy
```

This is one of Runact's major BEAM-inspired properties.

---

## 20. Actor Lifecycle Must Be Explicit

An actor has a lifecycle such as:

```text
Created
   ↓
Starting
   ↓
Running
   ↓
Waiting
   ↓
Running
   ↓
Stopping
   ↓
Stopped
```

**Failure:**

```text
Running
   ↓
Failed
   ↓
Supervisor
   ↓
Restart / Stop / Escalate
```

This becomes important for editor services and supervised processes.

The runtime tracks `ActorInfo` with `mailbox_depth` and logs lifecycle events via `tracing`.

---

## 21. No Actor Should Depend on Another Actor Staying Alive Forever

If:

```text
A → B
```

A must be able to handle:

- B stopped
- B restarted
- B unavailable
- B timed out

This encourages resilient systems.

When sending to a non-existent actor:

```rust
match runtime.send(ActorId::new(999), message) {
    Err(RuntimeError::ActorNotFound(id)) => {
        // Target actor does not exist
    }
    Err(RuntimeError::MailboxFull(id)) => {
        // Target mailbox is full
    }
    Ok(()) => { /* sent */ }
}
```

---

## 22. Backpressure Belongs at Boundaries

Suppose:

```text
AIActor
   ↓
10,000 requests
   ↓
ComputePool
```

The runtime should prevent unlimited work from accumulating.

Therefore:

```text
Actor
 ↓
bounded mailbox (default: 1000)
 ↓
Compute
```

If overloaded:

- Rejected (`MailboxFull` error)
- Busy (actor can't keep up)
- Deferred (poll with `try_recv`)
- Dropped (not currently supported — rejected by default)

The `send` API returns an error rather than silently dropping messages.

---

## 23. The Runtime Must Distinguish Three Kinds of Waiting

This is especially important for Runact.

| Kind | Behavior |
|------|----------|
| **Actor waiting** | Waiting for reply, timer, compute result — must not block a worker |
| **Compute waiting** | A CPU worker may legitimately be occupied doing CPU work |
| **External blocking** | An external OS thread may block if the API explicitly allows it |

So:

- Actor waiting → return from handler, poll later with `try_recv`
- CPU computation → occupy compute worker
- External blocking → allowed at boundary (`recv_timeout`, `send_blocking`)

---

## 24. The Fundamental Runact Invariant

> **An actor must never synchronously wait for another actor while occupying a Runact scheduler worker.**

And alongside it:

> **All actor-to-actor communication is asynchronous at the runtime level.**

Then:

> Synchronous request/reply is a convenience abstraction implemented through asynchronous messaging, a bounded reply channel, correlation ID, and optional timeout — not through blocking the scheduler.

That is the principle that differentiates Runact from a simple thread/channel library.

---

## The Resulting Model

### Runact Actor Communication

```text
                     RUNACT ACTOR
                          │
             ┌────────────┼────────────┐
             │            │            │
          Message      Request       Event
             │            │            │
             ▼            ▼            ▼
         Mailbox      Request ID     Mailbox
                          │
                          ▼
                       Reply
                          │
                          ▼
                    Resume Actor (poll result)
```

### Scheduler State Machine

```text
READY
  ↓
RUNNING
  ↓
WAITING ————————┐
  │             │
  │ reply/timer/compute result
  └———————→
           ↓
         READY
```

**No actor-level blocking. No lock-based actor communication. No forced cancellation. Asynchronous messaging first; synchronous-looking APIs only as a safe abstraction on top.**
