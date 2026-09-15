# Runact Object Model

## Overview

This document defines the core runtime objects in Runact. These objects form the public API surface. Implementation details must not leak through these types.

## Core Identifiers

### ProcessId

Stable, unique identifier for a process.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProcessId(u64);
```

- Created at spawn time
- Never reused within a runtime instance
- Serializable for persistence
- Orderable for debugging

### MessageId

Unique identifier for a message.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(u128);
```

- Generated for each message
- Used for correlation and deduplication

### CorrelationId

Links request/response pairs.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CorrelationId(u128);
```

- Set by sender
- Propagated through message chain
- Used for tracing and debugging

### TimerId

Stable identifier for a timer.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);
```

## Message Model

### MessageEnvelope

Container for all messages passing through the system.

```rust
pub struct MessageEnvelope {
    pub message_id: MessageId,
    pub correlation_id: Option<CorrelationId>,
    pub sender: ProcessId,
    pub recipient: ProcessId,
    pub protocol: ProtocolId,
    pub version: ProtocolVersion,
    pub priority: Priority,
    pub timestamp: Timestamp,
    pub payload: Vec<u8>,
}
```

### Priority

Message priority levels.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}
```

### ProtocolId

Identifies the message protocol.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProtocolId(u32);
```

### ProtocolVersion

Version of the protocol.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}
```

## Process

### ProcessInfo

Public view of a process.

```rust
pub struct ProcessInfo {
    pub id: ProcessId,
    pub name: Option<String>,
    pub state: ProcessState,
    pub parent: Option<ProcessId>,
    pub children: Vec<ProcessId>,
    pub mailbox_depth: usize,
    pub started_at: Timestamp,
    pub messages_processed: u64,
}
```

### ProcessState

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Spawned,
    Running,
    Stopping,
    Stopped,
    Terminated,
    Failed,
    Restarting,
}
```

## Supervisor

### SupervisorInfo

```rust
pub struct SupervisorInfo {
    pub id: ProcessId,
    pub strategy: RestartStrategy,
    pub children: Vec<ChildInfo>,
    pub restart_counts: HashMap<ProcessId, usize>,
}
```

### ChildInfo

```rust
pub struct ChildInfo {
    pub id: ProcessId,
    pub name: String,
    pub state: ProcessState,
    pub restart_policy: RestartPolicy,
    pub restart_count: usize,
}
```

### RestartStrategy

```rust
pub enum RestartStrategy {
    OneForOne,
    OneForAll,
    RestForOne,
}
```

### RestartPolicy

```rust
pub enum RestartPolicy {
    Permanent,
    Temporary,
    Transient,
}
```

## Mailbox

### MailboxInfo

```rust
pub struct MailboxInfo {
    pub capacity: usize,
    pub depth: usize,
    pub backpressure: BackpressurePolicy,
}
```

### BackpressurePolicy

```rust
pub enum BackpressurePolicy {
    Reject,
    Block,
    DropLowPriority,
}
```

## Timer

### TimerInfo

```rust
pub struct TimerInfo {
    pub id: TimerId,
    pub process_id: ProcessId,
    pub scheduled_at: Timestamp,
    pub interval: Option<Duration>,
}
```

## Design Principles

1. **No leaked types** — Tokio, serde, or other crate types must not appear in public APIs.
2. **Serializable** — All public types must be serializable for persistence.
3. **Cloneable** — All public types must be cloneable for inspection.
4. **Debuggable** — All public types must implement Debug.
5. **Stable IDs** — All identifiers must be stable across restarts.
