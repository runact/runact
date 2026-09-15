# Runact Protocol

## Overview

The Runact protocol defines how messages are structured, versioned, and transmitted between processes. It ensures interoperability between components and enables future evolution without breaking existing systems.

## Message Format

### MessageEnvelope

Every message in Runact is wrapped in a `MessageEnvelope`:

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

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `message_id` | `MessageId` | Unique identifier for this message |
| `correlation_id` | `Option<CorrelationId>` | Links request/response pairs |
| `sender` | `ProcessId` | Originating process |
| `recipient` | `ProcessId` | Target process |
| `protocol` | `ProtocolId` | Identifies the message protocol |
| `version` | `ProtocolVersion` | Version of the protocol |
| `priority` | `Priority` | Message priority level |
| `timestamp` | `Timestamp` | When the message was created |
| `payload` | `Vec<u8>` | Serialized message data |

## Protocol Identification

### ProtocolId

Identifies which protocol a message belongs to.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProtocolId(u32);
```

### Built-in Protocols

```rust
impl ProtocolId {
    pub const SYSTEM: Self = Self(0);
    pub const WORKSPACE: Self = Self(1);
    pub const BUFFER: Self = Self(2);
    pub const COMMAND: Self = Self(3);
    pub const EVENT: Self = Self(4);
    pub const TRANSACTION: Self = Self(5);
    pub const AGENT: Self = Self(6);
    pub const EXTENSION: Self = Self(7);
}
```

### Custom Protocols

Extensions define their own protocol IDs:

```rust
impl ProtocolId {
    pub fn custom(id: u32) -> Self {
        assert!(id >= 1000, "Custom protocol IDs must be >= 1000");
        Self(id)
    }
}
```

## Versioning

### ProtocolVersion

Semantic versioning for protocols.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}
```

### Compatibility Rules

- **Major version change** — Breaking changes. Not compatible.
- **Minor version change** — New features. Backward compatible.
- **Patch version change** — Bug fixes. Backward compatible.

### Version Negotiation

When processes communicate:

1. Sender includes protocol version in message
2. Receiver checks version compatibility
3. If incompatible, receiver rejects message with error
4. Error includes supported version range

```rust
pub enum ProtocolError {
    VersionMismatch {
        expected: ProtocolVersion,
        received: ProtocolVersion,
        supported_range: Range<ProtocolVersion>,
    },
    UnknownProtocol(ProtocolId),
    InvalidPayload { protocol: ProtocolId, error: String },
}
```

## Priority

### Priority Levels

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}
```

### Priority Usage

| Priority | Use Case |
|----------|----------|
| `Low` | Background tasks, logging |
| `Normal` | Standard messages |
| `High` | User-initiated actions |
| `Critical` | System shutdown, crash handling |

### Priority Handling

- Mailbox processes Critical messages first
- Then High, Normal, Low
- Within same priority, FIFO order

## Correlation

### Request/Response Pattern

```rust
// Sender creates correlation ID
let correlation_id = CorrelationId::new();
let request = MessageEnvelope {
    correlation_id: Some(correlation_id),
    // ...
};

// Sender sends request
sender.send(request).await?;

// Receiver processes and replies
let response = MessageEnvelope {
    correlation_id: Some(correlation_id),  // Same correlation ID
    sender: receiver_id,
    recipient: request.sender,
    // ...
};

receiver.send_response(response).await?;
```

### Correlation Usage

- Linking requests to responses
- Tracing message chains
- Debugging message flows
- Implementing timeouts

## Wire Format

### Serialization

Messages are serialized using a versioned binary format.

```rust
pub trait Codec: Send + Sync {
    fn encode(&self, message: &dyn Any) -> Result<Vec<u8>, CodecError>;
    fn decode(&self, data: &[u8]) -> Result<Box<dyn Any>, CodecError>;
    fn protocol_id(&self) -> ProtocolId;
    fn version(&self) -> ProtocolVersion;
}
```

### Default Codec

```rust
pub struct BincodeCodec {
    protocol_id: ProtocolId,
    version: ProtocolVersion,
}
```

### Extension Codecs

Extensions can register custom codecs:

```rust
runtime.register_codec(MyCustomCodec::new());
```

## Message Flow

### Direct Message

```
Process A → Mailbox B → Process B
```

### Request/Response

```
Process A → Mailbox B → Process B
    ↑                      │
    └──────────────────────┘
```

### Broadcast

```
Process A → Runtime → All Processes
```

### Dead Letter

```
Process A → Mailbox B (full) → Dead Letter Queue
```

## Dead Letters

When a message cannot be delivered:

1. Message is sent to dead letter queue
2. Dead letter handler is notified
3. Handler can log, retry, or discard

```rust
pub struct DeadLetter {
    pub envelope: MessageEnvelope,
    pub reason: DeadLetterReason,
    pub timestamp: Timestamp,
}

pub enum DeadLetterReason {
    MailboxFull,
    ProcessNotFound,
    ProcessStopped,
    VersionMismatch,
    InvalidPayload,
}
```

## Extension Protocol

### Extension Communication

Extensions communicate through a versioned protocol.

```rust
pub struct ExtensionMessage {
    pub extension_id: ExtensionId,
    pub protocol: ProtocolId,
    pub version: ProtocolVersion,
    pub payload: Vec<u8>,
}
```

### Protocol Negotiation

When an extension connects:

1. Extension sends hello with protocol version
2. Runtime checks compatibility
3. Runtime responds with supported version
4. Communication proceeds with negotiated version

```rust
pub struct ExtensionHello {
    pub extension_id: ExtensionId,
    pub protocol_version: ProtocolVersion,
    pub capabilities: Vec<Capability>,
}

pub struct ExtensionWelcome {
    pub supported_version: ProtocolVersion,
    pub granted_capabilities: Vec<Capability>,
}
```

## Security Considerations

### Message Authentication

- Messages include sender process ID
- Runtime validates sender exists
- Spoofed senders are rejected

### Capability Checking

- Messages requiring capabilities are checked
- Unauthorized messages are rejected with error

### Rate Limiting

- Mailboxes can enforce rate limits
- Excessive messages are rejected

## Observability

### Message Tracing

Every message includes:

- `message_id` — Unique identifier
- `correlation_id` — Links related messages
- `timestamp` — When created
- `sender` — Who sent it
- `recipient` — Who receives it

### Metrics

```rust
pub struct ProtocolMetrics {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub messages_rejected: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}
```
