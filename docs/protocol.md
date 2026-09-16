# Runact Protocol

## Overview

The Runact protocol defines how messages are structured and transmitted between actors. In v1.0.0, the protocol is simple: messages are sent through `crossbeam-channel` bounded queues with ownership transfer. There is no separate wire format or serialization for local actor-to-actor communication.

This document describes the current v1.0.0 protocol and future directions.

## Current Protocol (v1.0.0)

### MessageEnvelope

Every message in Runact is wrapped in a `MessageEnvelope`:

```rust
pub(crate) enum MessageEnvelope {
    Message(Box<dyn std::any::Any + Send>),
    Request {
        _request_id: u64,
        payload: Box<dyn std::any::Any + Send>,
        reply_sender: crossbeam_channel::Sender<Box<dyn std::any::Any + Send>>,
    },
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `Message` | `Box<dyn Any + Send>` | Fire-and-forget message |
| `Request.payload` | `Box<dyn Any + Send>` | Request message |
| `Request.reply_sender` | `crossbeam_channel::Sender` | Oneshot reply channel |

### Message Types

| Type | Behavior |
|------|----------|
| `Message` | Fire-and-forget — no reply expected |
| `Request` | Request/reply — includes reply channel |

### Sending

```rust
// Fire-and-forget
runtime.send(actor_id, message: M)?;

// Actor-to-actor
ctx.send_to(target, message: M)?;

// Request/reply
runtime.request(actor_id, message: M)?;
```

The `send` method uses `try_send` (non-blocking) and returns `RuntimeError::MailboxFull` if the bounded mailbox is full.

---

## Request/Response Pattern

### Correlation ID

```rust
let request_id = self.request_counter.fetch_add(1, Ordering::Relaxed);
```

Each request gets a unique monotonically-increasing ID.

### Reply Channel

```rust
let (reply_tx, reply_rx) = crossbeam_channel::bounded(1);
```

A bounded channel with capacity 1 is used for the reply. This prevents unbounded memory growth.

### RequestHandle

```rust
pub struct RequestHandle {
    id: u64,
    receiver: crossbeam_channel::Receiver<Box<dyn std::any::Any + Send>>,
}
```

### Usage

```rust
// External thread
let handle = runtime.request(actor_id, GetValue)?;
let reply = handle.recv_timeout(Duration::from_secs(5))?;
let value = *reply.downcast::<i64>()?;

// Inside actor handler
fn handle(&mut self, msg: MyMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
    if ctx.is_request() {
        ctx.reply(self.value)?;
    }
    Ok(())
}
```

---

## Backpressure

When a mailbox is full, the sender gets an explicit error:

```rust
match runtime.send(actor_id, message) {
    Err(RuntimeError::MailboxFull(id)) => {
        // Mailbox is full — caller decides what to do
    }
    Ok(()) => { /* message accepted */ }
}
```

No messages are silently dropped.

For external threads that can afford to block:

```rust
runtime.send_blocking(actor_id, message)?;
```

---

## Future Protocol Extensions

### Priority Levels

Planned:

```rust
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}
```

Mailbox would process Critical messages first, then High, Normal, Low (FIFO within priority).

### Versioned Envelopes

Planned for distributed actors:

```rust
pub struct MessageEnvelope {
    pub protocol: ProtocolId,
    pub version: ProtocolVersion,
    pub priority: Priority,
    pub correlation_id: Option<CorrelationId>,
    pub timestamp: Instant,
    pub payload: Vec<u8>,
}
```

### Wire Format

Planned for extension processes and distributed runtime:

- Binary serialization (bincode-like)
- Protocol versioning
- Custom codecs for extensions

These are future considerations. The v1.0.0 protocol is intentionally simple.

---

## Security Considerations

### Message Authentication

- Messages include the sending actor's identity (via `ActorId`)
- The runtime validates that target actors exist
- Spoofed sender IDs are not possible (the runtime assigns IDs)

### Rate Limiting

- Bounded mailboxes provide implicit rate limiting
- Compute pool queue capacity provides backpressure
- Future: explicit per-actor rate limiting

### Capability Checking

- Resource access is mediated by `Capability<H>` wrappers
- Actors can only use capabilities they own or have been granted
- Compute tasks are isolated — panics don't affect the actor

---

## Metrics

Current observability:

```rust
pub struct RuntimeStats {
    pub actor_count: usize,
    pub request_count: u64,
}
```

Via `tracing`:

- `info` — actor spawned, stopped
- `debug` — message sent, request sent
- `trace` — per-message handling
- `warn` — actor restarted
- `error` — max restarts exceeded
