# Chapter 6: Mailboxes and Backpressure

Every actor has a **mailbox** — a bounded queue that buffers incoming
messages. This chapter explains mailbox mechanics, backpressure, and the
reduction system that ensures fair scheduling.

---

## 6.1 What Is a Mailbox?

When you call `runtime.send(id, msg)` or `ctx.send_to(id, msg)`, the
message is not delivered to the actor immediately. Instead, it is pushed
onto the target actor's **mailbox** — a FIFO queue backed by a
`crossbeam_channel::bounded` sender.

```
Sender ──message──► [mailbox: bounded queue] ──► Actor (when scheduled)
```

The actor processes messages one at a time, in order. While the actor is
processing message N, messages N+1, N+2, ... wait in the queue.

## 6.2 Bounded Mailboxes

By default, mailboxes are **bounded**. The capacity is configurable via
`RuntimeConfig`:

```rust
use runact::{Runtime, RuntimeConfig};

let config = RuntimeConfig {
    mailbox_capacity: 512,  // default: 1024
    ..RuntimeConfig::default()
};
let mut runtime = Runtime::with_config(config).unwrap();
```

When the mailbox is full and you try to send:

```rust
match runtime.send(actor_id, message) {
    Ok(()) => { /* delivered */ }
    Err(RuntimeError::MailboxFull(id)) => {
        // The actor's mailbox is at capacity.
        // Options: retry, drop, or use send_blocking
    }
    Err(e) => { /* other error */ }
}
```

## 6.3 Backpressure

**Backpressure** is the mechanism by which a slow consumer signals a fast
producer to slow down. Runact's bounded mailboxes provide natural
backpressure:

- If an actor processes messages slower than they arrive, the mailbox
  fills up.
- Once full, `send` returns `MailboxFull` immediately — the sender gets
  feedback and must handle it.
- This prevents unbounded memory growth.

### Handling MailboxFull

```rust
// Retry with backoff
loop {
    match runtime.send(actor_id, msg.clone()) {
        Ok(()) => break,
        Err(RuntimeError::MailboxFull(_)) => {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        Err(e) => return Err(e),
    }
}
```

### send vs send_blocking

| Method | Behavior when full | Use case |
|--------|--------------------|----------|
| `send` | Returns `MailboxFull` immediately | Non-blocking, retry loop |
| `send_blocking` | Blocks until accepted | External threads that can afford to wait |

Inside actors, use `ctx.send_to` (always non-blocking). From external
threads, choose `send` (fast-fail) or `send_blocking` (wait).

## 6.4 Reduction Counting

Runact's scheduler uses **reduction counting** — a technique borrowed from
BEAM. Each actor gets a budget of "reductions" (work units) per scheduling
turn. When the budget is exhausted, the actor yields, and another actor
gets scheduled.

### How Reductions Work

```
Actor A: [msg1] → handle() → reduction budget-- → msg2 → handle() → budget exhausted
Scheduler: yield Actor A, schedule Actor B
Actor B: [msg3] → handle() → ... 
```

The default budget is `MAX_REDUCTIONS` (typically a few hundred messages
per turn). This ensures:
- **Fairness**: No single actor can monopolize a worker thread.
- **Responsiveness**: All actors get a turn within a bounded time.
- **Throughput**: Each actor processes multiple messages per scheduling
  turn (amortizing the context switch cost).

### Why It Matters

Without reduction counting, a busy actor could process thousands of
messages in a single scheduling turn, starving all other actors. With
reductions, the runtime guarantees that N actors share N worker threads
fairly, even if one actor is under heavy load.

## 6.5 Idle Actors

When an actor's mailbox is empty, the actor is **parked** — it is
removed from the scheduler's run queue. Parked actors consume **zero**
scheduler resources.

When a new message arrives:

```
Message arrives ──► mailbox (was empty)
    │
    ▼
Scheduler: actor is now "ready" — push onto run queue
    │
    ▼
Worker polls actor: handle(msg) for up to MAX_REDUCTIONS messages
```

This is why Runact can have hundreds of thousands of actors that spend
most of their time idle — they cost nothing when parked, and wake up
instantly when a message arrives.

## 6.6 Mailbox Internals

The mailbox is implemented as a `BoundedMailbox` struct (private to the
runtime):

```rust
// Simplified — not public API
struct BoundedMailbox {
    sender: crossbeam_channel::Sender<MessageEnvelope>,
    receiver: crossbeam_channel::Receiver<MessageEnvelope>,
    capacity: usize,
}
```

Each actor has exactly one mailbox. The `ActorId` →
`sender` mapping is stored in the runtime's global registry:

```rust
// In Runtime
actors: Arc<RwLock<HashMap<ActorId, ActorInfo>>>,
senders: Arc<RwLock<HashMap<ActorId, crossbeam_channel::Sender<MessageEnvelope>>>>,
```

This is how `send` and `send_to` locate the target's mailbox without
knowing the worker thread.
