# Runact Runtime + Networking Plan

## 1. Objective

Extend Runact into a small, Rust-native concurrency runtime capable of powering:

1. PaperOS desktop applications
2. Remote AI-agent servers
3. Long-running agent processes
4. Async network services
5. CPU-heavy agent workloads
6. External tool execution

Runact must remain a general-purpose runtime.

It must NOT become an HTTP framework, WebSocket framework, TLS library, or application framework.

Core principle:

> Runact manages execution, concurrency, lifecycle, and low-level I/O readiness. Higher-level libraries implement protocols and applications.

---

## 2. Target Architecture

```text
                         PaperOS Desktop
                              │
                         Agent Client
                              │
                       HTTPS / WebSocket
                              │
                              ▼
                    Remote Agent Server
                              │
                       HTTP / WebSocket
                              │
                              ▼
                            Runact
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
      Actors              Async Tasks          Compute Pool
        │                     │                     │
        └─────────────────────┼─────────────────────┘
                              │
                         I/O Runtime
                              │
                           TCP
                              │
                    Linux epoll initially
```

---

## 3. Runact Responsibilities

Runact owns:

```text
Actor system
Scheduler
Async tasks
Future polling
Wakers
Cancellation
Timers
Task groups
Supervision
Compute pool
TCP
I/O readiness
Process lifecycle
Backpressure
Runtime shutdown
```

Runact does NOT own:

```text
HTTP
WebSocket
TLS
DNS
HTTP client
HTTP server
LSP protocol
Git protocol
AI provider APIs
JSON-RPC protocol
Application-level agent protocol
```

Those belong to higher-level libraries or applications.

---

## 4. Crate Architecture

Prefer separating the system into crates.

```text
runact/
├── runact-core/
│   ├── actor/
│   ├── mailbox/
│   ├── scheduler/
│   ├── supervision/
│   └── lifecycle/
│
├── runact-runtime/
│   ├── executor/
│   ├── task/
│   ├── waker/
│   ├── cancellation/
│   ├── timer/
│   ├── task_group/
│   └── shutdown/
│
├── runact-compute/
│   ├── pool/
│   └── job/
│
├── runact-net/
│   ├── tcp/
│   ├── reactor/
│   ├── readiness/
│   └── socket/
│
└── runact-process/
    ├── process/
    ├── stdin/
    ├── stdout/
    ├── stderr/
    └── cancellation/
```

The exact crate boundaries may change during implementation, but the conceptual separation should remain.

---

## 5. Async Runtime

Runact must execute standard Rust futures.

Use:

```rust
Future
Poll
Context
Waker
Pin
```

Do NOT create a custom async programming model.

Basic API:

```rust
runtime.spawn(future)
runtime.sleep(duration)
runtime.timeout(duration, future)
runtime.compute(job)
runtime.task_group()
runtime.shutdown()
```

---

## 6. Task Lifecycle

Implement:

```text
Created
   ↓
Scheduled
   ↓
Running
   ↓
Waiting
   │
   └──── wake ────→ Runnable
                         │
                         ▼
                      Running
                         │
              ┌──────────┴──────────┐
              ▼                     ▼
          Completed               Failed
```

Cancellation:

```text
Running / Waiting
       ↓
 Cancelling
       ↓
 Cancelled
```

A pending task must not continuously consume worker time.

---

## 7. Waker

Implement a Runact-specific Waker.

When a future returns:

```rust
Poll::Pending
```

the Waker must arrange for the task to become runnable again.

Conceptual flow:

```text
Future
  ↓
Poll::Pending
  ↓
Waker registered
  ↓
External event
  ↓
Waker.wake()
  ↓
Task marked runnable
  ↓
Scheduler queue
  ↓
Worker polls task
```

Prevent duplicate scheduling with an atomic runnable state.

---

## 8. Task Scheduling

The scheduler must provide:

* fairness
* bounded queues where appropriate
* wake-up efficiency
* no starvation
* task cancellation
* graceful shutdown

Do not let one CPU-heavy task block normal async work.

---

## 9. Actor Integration

Actors remain distinct from async tasks.

Actors:

```text
Mailbox
   ↓
Scheduler
   ↓
Actor execution
   ↓
Yield
```

An actor must not block waiting for network I/O.

Instead:

```text
Actor
  │
  ├── spawn async network task
  │
  └── continue processing messages
             │
             ▼
       network completes
             │
             ▼
        actor message
             │
             ▼
        actor processes response
```

Example:

```text
FetchAgentResponse
        ↓
spawn HTTP task
        ↓
Actor continues
        ↓
HTTP completes
        ↓
AgentResponseReceived
        ↓
Actor processes response
```

---

## 10. Cancellation

Implement:

```rust
CancellationToken
```

Required operations:

```rust
cancel()
is_cancelled()
child_token()
```

Cancellation must be cooperative.

Do not forcibly kill arbitrary threads.

Cancellation must propagate through structured task groups.

---

## 11. Task Groups

Implement structured concurrency.

Example:

```text
AgentSession
    │
    └── TaskGroup
         ├── LLM request
         ├── tool execution
         ├── network connection
         └── timeout task
```

When an agent session terminates:

```text
AgentSession closed
       ↓
Cancel TaskGroup
       ↓
Cancel children
       ↓
Wait for children
       ↓
Release resources
```

This is particularly important for remote AI agents.

---

## 12. Timers

Implement:

```rust
runtime.sleep(duration)
runtime.timeout(duration, future)
```

Start with a simple correct timer implementation.

Do not optimize prematurely.

A timer wheel can be added later if profiling proves it necessary.

---

## 13. Compute Pool

CPU-heavy operations must not execute on ordinary async workers.

Provide:

```rust
runtime.compute(|| {
    expensive_operation()
});
```

Use cases:

```text
code indexing
search
parsing
syntax analysis
compression
hashing
large JSON processing
AI preprocessing
diff generation
```

Architecture:

```text
Runact
 ├── Actor workers
 ├── Async workers
 └── Compute workers
```

Compute work must not starve actor or async execution.

---

## 14. Minimal Network Runtime

Implement a minimal `runact-net`.

The initial goal is NOT to build a networking framework.

The goal is:

> Give Runact native async TCP and OS readiness integration.

Initial API:

```rust
TcpListener
TcpStream
```

Required operations:

```rust
TcpListener::bind()
TcpListener::accept()

TcpStream::connect()
TcpStream::read()
TcpStream::write()
TcpStream::shutdown()
```

Also provide:

```text
socket address
connection state
read readiness
write readiness
connection cancellation
```

---

## 15. I/O Reactor

This is the most important part of `runact-net`.

Architecture:

```text
TcpStream
    │
    ▼
OS socket
    │
    ▼
OS readiness API
    │
    ▼
Reactor
    │
    ▼
Runact Waker
    │
    ▼
Scheduler
    │
    ▼
Future resumes
```

Linux is the first target.

Use:

```text
epoll
```

Do not initially attempt:

```text
kqueue
IOCP
io_uring
```

Build a correct Linux implementation first.

Design the reactor abstraction so other platforms can be added later.

---

## 16. TCP Scope

Initial `runact-net` supports:

```text
TCP client
TCP server
async read
async write
partial read
partial write
connection close
timeouts
cancellation
backpressure
```

Do NOT implement:

```text
HTTP
WebSocket
TLS
DNS
HTTP/2
HTTP/3
QUIC
proxy
connection pooling
```

These remain outside Runact.

---

## 17. Backpressure

Network writes must not create unlimited memory growth.

Provide bounded buffering or explicit write-pressure handling.

Example:

```text
Agent output
     ↓
bounded channel
     ↓
WebSocket/HTTP layer
     ↓
TCP
```

If the remote client becomes slow, the agent must not allocate unlimited output buffers.

---

## 18. Process Runtime

AI coding agents need process execution.

Implement a separate process abstraction.

Required capabilities:

```text
spawn process
stdin
stdout
stderr
exit status
timeout
cancellation
graceful termination
```

Example:

```text
Agent
  ↓
run cargo test
  ↓
Process
  ├── stdout
  ├── stderr
  └── exit status
```

Runact should manage process lifecycle.

The agent layer decides what commands are allowed.

---

## 19. Process Cancellation

When:

```text
Agent cancelled
```

propagate:

```text
Agent Task
   ↓
Tool Task
   ↓
Process
```

The process must not remain orphaned after the agent terminates.

---

## 20. Remote AI Agent Architecture

The remote server should eventually look like:

```text
                    Agent Server
                         │
                    Runact Runtime
                         │
              ┌──────────┼──────────┐
              │          │          │
          AgentActor   Tasks      Compute
              │          │          │
       ┌──────┼──────────┼──────────┤
       │      │          │          │
      LLM   Tools      Files      Process
       │
       ▼
   HTTP Client
       │
       ▼
  AI Provider
```

The HTTP client itself should be implemented outside Runact.

Runact provides the async execution environment.

---

## 21. Agent Connection Architecture

The remote server can eventually expose:

```text
HTTP
WebSocket
```

but those protocols remain outside Runact.

Architecture:

```text
Internet
   │
   ▼
HTTP/WebSocket Server
   │
   ▼
Agent Gateway
   │
   ▼
Runact
   │
   ▼
AgentActor
```

The gateway translates network messages into agent commands.

---

## 22. Agent Cancellation Flow

This must work end-to-end:

```text
PaperOS
   │
   │ cancel agent
   ▼
Agent Gateway
   │
   ▼
AgentActor
   │
   ▼
CancellationToken
   │
   ├── cancel LLM task
   ├── cancel tool task
   ├── cancel network task
   └── terminate process
```

This is one of the main reasons Runact needs structured cancellation.

---

## 23. PaperOS Desktop

PaperOS should NOT require a network server just to operate locally.

Architecture:

```text
PaperOS Desktop
   │
   ├── WebView
   │
   ├── PaperOS Core
   │
   └── Runact
```

The desktop UI communicates with PaperOS through the local UI bridge.

Remote AI is optional:

```text
PaperOS
   │
   └── Agent Client
          │
          │ HTTPS/WSS
          ▼
       AI Server
```

Therefore `runact-net` is not required merely to render the PaperOS desktop UI.

---

## 24. Remote Agent Client

PaperOS should eventually have an agent client abstraction:

```rust
trait AgentClient {
    async fn start(...);
    async fn send(...);
    async fn cancel(...);
}
```

Implementations can include:

```text
LocalAgent
RemoteAgent
```

The UI does not need to know which one is being used.

---

## 25. Runtime Adapter Principle

Runact should remain independent of higher-level protocol implementations.

Architecture:

```text
Runact
   │
   ├── TCP
   │
   ├── Process
   │
   ├── Futures
   │
   └── Scheduler
          │
          ▼
Higher-level libraries
   │
   ├── HTTP
   ├── WebSocket
   ├── TLS
   └── DNS
```

Do not add protocol-specific APIs to Runact.

---

## 26. Tokio Decision

Do not make Tokio a mandatory Runact dependency.

If an external library requires Tokio, create an adapter:

```text
runact-tokio
```

Possible architecture:

```text
Runact
   │
   ├── native runtime
   │
   └── optional adapters
          └── Tokio
```

The long-term goal is that Runact can operate without Tokio.

For the first native networking implementation, do not introduce Tokio just to implement TCP.

---

## 27. Development Phases

Development phases are tracked in [Roadmap](roadmap.md). The roadmap is the authoritative source for phase numbering and status.

This document describes the architecture and design for the runtime + networking extension. For implementation status, see the roadmap.

### Phase 9 — Runtime + Networking Extension (roadmap.md)

This phase extends Runact with TCP networking, process runtime, cancellation, task groups, and async timers. All sub-phases are complete:

- **9a** — Cancellation & Task Groups ✅
- **9b** — Timers ✅
- **9c** — Actor ↔ Async Integration ✅
- **9d** — Process Runtime ✅
- **9e** — TCP Reactor ✅
- **9f** — Stress Testing ✅

### Next: Phase 12 — WebSocket (roadmap.md)

Phase 10 (HTTP) and Phase 11 (Web Framework) are complete in `runact-web`.
Phase 12 (WebSocket) is complete: RFC 6455 frame parsing/encoding, handshake
validation, `WebSocketServer` generic over stream type, `AsyncWebSocket` with
non-blocking reader/writer/ping threads, `RunactTcpStream` adapter bridging
runact's `TcpStream`, and example binaries.

---

## 28. Performance Goals

Do not optimize before measuring.

Initial goals:

```text
No blocking actor workers
No blocking async workers
Bounded memory under slow clients
Low scheduling overhead
Efficient wakeups
Predictable cancellation
No orphaned tasks
No orphaned processes
```

Measure:

```text
task scheduling latency
wake latency
TCP readiness latency
throughput
memory per connection
CPU utilization
actor fairness
```

---

## 29. Testing Strategy

Unit tests:

```text
Future
Waker
Task
Cancellation
Timer
TaskGroup
Actor integration
TCP
Reactor
Process
```

Integration tests:

```text
TCP client/server
multiple connections
cancellation
timeouts
process execution
agent lifecycle
```

Stress tests:

```text
10k tasks
10k wakeups
1k TCP connections
slow clients
large concurrent workloads
mixed actors + tasks + TCP + compute
```

Failure tests:

```text
actor panic
future panic
connection reset
process crash
client disconnect
LLM timeout
task cancellation
runtime shutdown
```

---

## 30. Architectural Invariants

The canonical invariant set is in [Architecture](architecture.md) §35. The networking-relevant invariants are:

1. Runact is not an HTTP framework.
2. Runact is not a WebSocket framework.
3. Runact is not a TLS library.
4. Runact is not an AI framework.
5. Runact executes standard Rust futures.
6. Actors and async tasks remain distinct concepts.
7. CPU-heavy work uses the compute pool.
8. Async I/O never blocks an actor worker.
9. Cancellation is cooperative.
10. Structured concurrency prevents orphaned tasks.
11. Process cancellation prevents orphaned child processes.
12. TCP is the lowest-level network primitive in Runact.
13. HTTP/WebSocket remain outside Runact.
14. Tokio is optional, not fundamental.

See [Architecture §35](architecture.md#35-architectural-invariants) for the full set (27 invariants).

---

## 31. Final Target

The final conceptual architecture is:

```text
                         PAPEROS
                            │
             ┌──────────────┴──────────────┐
             │                             │
        Desktop UI                    Remote Agent
             │                             │
          WebView                    HTTPS / WSS
             │                             │
             └──────────────┬──────────────┘
                            │
                       PAPEROS CORE
                            │
                          RUNACT
                            │
       ┌────────────────────┼────────────────────┐
       │                    │                    │
     Actors              Async Tasks         Compute
       │                    │                    │
       ├────────────────────┼────────────────────┤
       │                    │                    │
    Process              Timers             Cancellation
       │
       └────────────────────┐
                            │
                         I/O Runtime
                            │
                           TCP
                            │
                         epoll
```

The guiding principle is:

> **Runact should provide the machinery required to safely and efficiently run concurrent work. It should not provide the application protocols that use that machinery.**

Build the smallest runtime that can power PaperOS and a remote AI-agent server, then expand only when a real workload proves that a new primitive belongs in Runact.
