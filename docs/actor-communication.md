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

Rust's ownership system enforces this wherever possible.

## 2. Message Sending is Asynchronous

The fundamental operation is:

```rust
actor.send(message);
```

It means:

> "Put this message into the recipient's mailbox and return."

It does **not** mean:

> "Wait for the recipient to process this message."

Therefore:

```rust
a.send(Message::Something);
```

should normally return immediately.

## 3. `send()` Must Not Wait for the Actor

This is an important distinction.

**Bad:**

```rust
a.send(msg); // blocks until A processes msg
```

**Correct:**

```rust
a.send(msg); // enqueue and return
```

The sender should not depend on the recipient's execution speed.

## 4. Request/Reply is Still Asynchronous

Sometimes an actor needs an answer.

Don't turn that into synchronous actor blocking.

Instead:

```rust
let handle = actor.request(GetUser { id });
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

The request returns a handle, not the result itself.

For example:

```rust
let result = actor.request(message);
result.on_reply(|reply| {
    // handle reply
});
```

Or, if Runact eventually has an actor-native async mechanism:

```rust
let result = request.await;
```

But the crucial question is:

> What happens to the actor while it waits?

It must remain schedulable.

## 5. An Actor Must Never Block Its Scheduler Worker

This is probably the most important runtime rule.

**Bad:**

```
Worker 1
   │
   └── Actor A
          │
          └── waiting for B
```

Worker 1 should not become:

```
Worker 1
   │
   └── BLOCKED
```

**Correct:**

```
Actor A
   │
   └── waiting for reply
          │
          ↓
      scheduler
          │
          ├── Actor B
          ├── Actor C
          └── Actor D
```

A waiting actor becomes Waiting/Suspended, while the worker executes something else.

## 6. Waiting is a State, Not Blocking

Think of an actor state machine:

```
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
       Ready      Waiting   Sleeping
                    │
                    │ reply
                    ▼
                  Ready
```

The actor can wait for:

- reply
- timer
- I/O
- resource
- external event

without blocking an OS thread.

## 7. Actor-to-Actor Calls Should Not Hold Locks

**Avoid:**

```
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

## 8. Request/Reply Must Have Correlation IDs

A request should carry an ID.

**Request:**

```
Request
├── request_id
├── sender
└── payload
```

**Reply:**

```
Reply
├── request_id
├── sender
└── result
```

**Example:**

```
A ── request #42 ──> B
A ── request #43 ──> B

B ── reply #43 ──> A
B ── reply #42 ──> A
```

The runtime can match replies correctly.

This becomes important when an actor has many outstanding requests.

## 9. Timeouts are Optional, Not Implicit

A request may specify:

```rust
request.timeout(Duration::from_secs(5));
```

If the timeout expires:

```
Request
   │
   ├── Reply → success
   │
   └── Timeout → failure
```

But Runact should not automatically impose arbitrary timeouts.

A local computation might legitimately take 30 seconds.

Timeouts are application-level policy.

## 10. Cancellation Should Be Cooperative

Never try to forcibly kill arbitrary Rust code.

**Bad model:**

```
cancel()
   ↓
kill thread
```

**Instead:**

```
cancel()
   ↓
CancellationToken
   ↓
computation checks token
   ↓
stops safely
```

For example:

```rust
if ctx.cancelled() {
    return Err(Error::Cancelled);
}
```

This matters particularly for the Runact compute pool.

## 11. Fire-and-Forget is a First-Class Pattern

Sometimes no response is required.

```rust
logger.send(Log(message));
```

The sender doesn't care about a reply.

This should be extremely cheap.

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

```
FileActor
   │
   └── FileChanged
           │
           ├── BufferActor
           ├── UIActor
           └── SearchActor
```

rather than having everyone synchronously query the file actor.

This encourages loose coupling.

## 13. Don't Make Every Operation Request/Reply

**Bad architecture:**

```
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

```
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

## 14. Mailbox Backpressure Must Be Explicit

Asynchronous doesn't mean unlimited queues.

Consider:

```
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

Therefore Runact needs mailbox policies.

**Potential policies:**

- Bounded
- Drop
- Reject
- Block external producer
- Coalesce
- Priority

But blocking an actor because its mailbox is full should not be the default.

For v0.1:

> Use bounded mailboxes where appropriate and return an explicit error when capacity is exhausted.

## 15. External Threads Are Different

There is an important distinction between:

- Actor → Actor
- External OS thread → Actor

A normal OS thread can potentially block.

For example:

```rust
let result = actor.request(msg).wait();
```

could be acceptable from an external thread.

But:

```rust
actor_context.call(other_actor, msg).wait();
```

inside an actor should be discouraged or prohibited.

So Runact can have two APIs:

```rust
// Actor context
request(actor, message)

// External/blocking context
request_blocking(actor, message)
```

This distinction makes the semantics clear.

## 16. Actor Code Should Be Mostly Deterministic

An actor should conceptually behave like:

```
State + Message → New State + Effects
```

For example:

```rust
fn handle(
    state: &mut State,
    message: Message,
) -> Effects {
    // ...
}
```

The actor owns the state transition.

External effects should go through runtime services.

This makes actors easier to test.

## 17. Effects Should Be Explicit

An actor shouldn't directly do everything.

**Good:**

```
BufferActor
   │
   ├── state mutation
   │
   └── request FileActor to save
```

**Bad:**

```
BufferActor
   │
   └── directly manipulates filesystem
```

This produces clearer architecture.

## 18. Long Computations Are Not Actor Work

Suppose an actor receives:

```
CompileProject
```

It shouldn't perform a 30-second compilation inside its actor execution.

Instead:

```
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

This preserves responsiveness.

## 19. Failure is a Message/Runtime Event

An actor failing should not mean:

```
panic
 ↓
Runact dies
```

**Instead:**

```
Actor
  │
  X
  │
  ▼
Runtime
  │
  ▼
Supervisor
  │
  ├── restart
  ├── stop
  └── escalate
```

This is one of Runact's major BEAM-inspired properties.

## 20. Actor Lifecycle Must Be Explicit

An actor should have a lifecycle such as:

```
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

```
Running
   ↓
Failed
   ↓
Supervisor
   ↓
Restart / Stop / Escalate
```

This becomes important for editor services.

## 21. No Actor Should Depend on Another Actor Staying Alive Forever

If:

```
A → B
```

A must be able to handle:

- B stopped
- B restarted
- B unavailable
- B timed out

This encourages resilient systems.

## 22. Backpressure Belongs at Boundaries

Suppose:

```
AIActor
   ↓
10,000 requests
   ↓
ComputePool
```

The runtime should prevent unlimited work from accumulating.

Therefore:

```
Actor
 ↓
bounded queue
 ↓
Compute
```

If overloaded:

- Rejected
- Busy
- Deferred
- Dropped

depending on the API.

## 23. The Runtime Must Distinguish Three Kinds of Waiting

This is especially important for Runact.

| Kind | Behavior |
|---|---|
| **Actor waiting** | Waiting for reply, timer, I/O — must not block a worker |
| **Compute waiting** | A CPU worker may legitimately be occupied doing CPU work |
| **External blocking** | An external OS thread may block if the API explicitly allows it |

So:

- Actor waiting → suspend actor
- CPU computation → occupy compute worker
- External blocking → allowed at boundary

## 24. The Fundamental Runact Invariant

> **An actor must never synchronously wait for another actor while occupying a Runact scheduler worker.**

And alongside it:

> **All actor-to-actor communication is asynchronous at the runtime level.**

Then:

> Synchronous request/reply is a convenience abstraction implemented through asynchronous messaging, suspension, correlation, and optional timeout — not through blocking the scheduler.

That is the principle that differentiates Runact from a simple thread/channel library.

---

## The Resulting Model

### Runact Actor Communication

```
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
                    Resume Actor
```

### Scheduler State Machine

```
READY
  ↓
RUNNING
  ↓
WAITING ───────────────┐
  │                    │
  │ reply/timer/I/O     │
  └────────────────────┘
           ↓
         READY
```

**No actor-level blocking. No lock-based actor communication. No forced cancellation. Asynchronous messaging first; synchronous-looking APIs only as a safe abstraction on top.**
