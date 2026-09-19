# Chapter 2: Core Concepts

Before writing your first actor, you need to understand five foundational
concepts that Runact builds on. This chapter explains each one and how
they fit together.

---

## 2.1 Actor

An **actor** is a self-contained unit of state and behavior. Each actor:

- Owns its private state (fields on the struct).
- Defines a single message type it accepts.
- Implements the `Actor` trait, which provides a `handle` method called for
  every incoming message.

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

**Key rules:**
- An actor's `handle` must be **synchronous** and **non-blocking**. Do not
  call `std::thread::sleep`, do not block on I/O, do not loop forever.
- When an actor receives a message, it processes that message to
  completion before moving to the next one.
- Actors never share mutable memory. Communication is exclusively through
  messages, which are **moved** (not borrowed).

## 2.2 ActorId

Every spawned actor is assigned an `ActorId` — a unique identifier within
the runtime. You use it to address the actor when sending messages.

```rust
let id: ActorId = runtime.spawn(MyActor).unwrap();
runtime.send(id, some_message()).unwrap();
```

`ActorId` is:
- `Clone` — you can hand copies to other actors.
- `Copy` — it is just a `u64` wrapper.
- `Debug`, `Hash`, `Eq` — usable as a key in `HashMap`.

### Addressing Without IDs

If an actor needs to send messages to another actor regularly, store the
`ActorId` as a field:

```rust
struct Client {
    server: ActorId,
}

impl Actor for Client {
    type Message = String;
    fn handle(&mut self, msg: String, ctx: &mut ActorContext) -> Result<(), ActorError> {
        // Forward the message to the server
        ctx.send_to(self.server, msg)?;
        Ok(())
    }
}
```

## 2.3 Mailbox

Each actor has a **mailbox** — a bounded queue of incoming messages. When
you call `runtime.send(id, msg)`, the message is pushed onto the actor's
mailbox. The scheduler picks up the actor when it has messages waiting and
polling budget remaining.

### Bounded Mailboxes and Backpressure

By default, Runact uses a **bounded mailbox** (capacity configurable via
`RuntimeConfig`). When the mailbox is full:

```
Sender ──send()──► Mailbox (full) ──reject──► Caller gets RuntimeError::MailboxFull
```

This is **backpressure**: the sender is forced to slow down when the
receiver can't keep up. This prevents unbounded memory growth.

If you need fire-and-forget with backpressure, check for
`RuntimeError::MailboxFull` and retry with a delay, or drop the message
gracefully.

### Reduction Counting

Each actor processes a bounded number of messages per scheduling quantum
(a "reduction" budget, similar to BEAM's reduction counting). When the
budget is exhausted, the actor yields, and another actor gets scheduled.
This ensures fair scheduling — no single actor can monopolize a worker
thread.

## 2.4 Runtime

The **`Runtime`** is the top-level coordinator. It owns:

- The **scheduler** (assigns actors to worker threads)
- The **compute pool** (offloads CPU-bound work)
- The **task executor** (runs async `Future`s)
- The **timer service** (schedules delayed/periodic messages)
- The **actor registry** (maps `ActorId` → mailbox senders)

You create a runtime with `Runtime::new()` and tear it down with
`runtime.shutdown()`.

```rust
let mut runtime = Runtime::new().unwrap();
// ... spawn actors, send messages ...
runtime.shutdown();
```

### RuntimeSender

For sending messages from **outside** the actor system (e.g., from a
callback, a thread, or an async task), use `RuntimeSender`:

```rust
let sender = runtime.sender();
sender.send(actor_id, message).unwrap();
```

`RuntimeSender` is `Clone`, so you can give it to any thread or async task
that needs to message an actor.

## 2.5 Supervision

**Supervision** is Runact's fault-tolerance mechanism. A `Supervisor`
spawns child actors and defines what happens when a child panics:

| Strategy    | On child panic           |
|-------------|--------------------------|
| `OneForOne` | Restart only the child   |
| `RestForOne`| Restart child + subsequent siblings |

Supervisors use **exponential backoff**: after each restart, the delay
doubles (up to a cap). If a child crashes too many times within a window,
the supervisor gives up and the whole subtree is torn down.

Supervision ties together with the actor lifecycle: a child actor receives
an `ActorContext::init` call on startup (including restarts), so it can
re-initialize its state cleanly.

## 2.6 Async Tasks vs. Actors

Runact has two concurrency primitives:

| Feature           | Actor                          | Async Task                     |
|-------------------|--------------------------------|--------------------------------|
| Concurrency unit  | `Actor` trait impl             | `Future`                        |
| Communication     | Messages (`send`, `request`)   | Return value (`TaskHandle::recv`) |
| Scheduling        | Reductions (cooperative)      | Task budget (cooperative)       |
| Error handling    | Panic → supervisor restart     | `Result<T, TaskError>`           |
| Use case          | Stateful services, long-lived  | Short-lived I/O, one-shot work  |

**Rule of thumb:** If your unit of work has identity and long-lived state,
use an **actor**. If it's a short-lived computation (e.g., an HTTP request
handler), use an **async task**.

## 2.7 The Runtime Architecture

```
┌──────────────────────────────────────────────┐
│              Runtime (top-level)              │
├──────────────────────────────────────────────┤
│  Scheduler  │  Async Executor  │  Compute   │
│  (actors)   │  (Futures)       │  Pool      │
├─────────────┴─────────┬────────┴────────────┤
│     Timer Service     │     Actor Registry  │
├───────────────────────┴─────────────────────┤
│              TCP / I/O Reactor               │
└──────────────────────────────────────────────┘
```

The scheduler and async executor share worker threads but use **reduction
budgeting** to ensure fairness between actors and async tasks. CPU-bound
work is offloaded to the compute pool so it never blocks a worker.
