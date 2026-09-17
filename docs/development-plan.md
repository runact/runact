# Runact — Full Development Plan

## 1. Project Vision

Build **Runact** as a Rust-native concurrent runtime and application platform inspired by BEAM, while taking advantage of Rust's ownership, type system, RAII, and zero-cost abstractions.

Runact should eventually provide the foundation for:

* Web applications
* AI agents
* Remote AI agent servers
* PaperOS
* Network services
* Background workers
* CLI applications
* Real-time applications
* Automation systems

The key idea:

> **Runact is one ecosystem, with multiple layers and crates.**

Do not build `runact-web` as an unrelated project.

---

# 2. Overall Architecture

```text
                         RUNACT
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
        ▼                  ▼                  ▼
     Runtime              Web              Process
        │                  │                  │
        │              HTTP/WebSocket        │
        │                  │                  │
        └──────────────────┼──────────────────┘
                           │
                           ▼
                      Runact Net
                           │
                           ▼
                      Operating System
```

More precisely:

```text
Applications
     │
     ├── PaperOS
     ├── AI Agents
     ├── Web Applications
     └── Other Services
     │
     ▼
Runact Application APIs
     │
     ├── Web
     ├── Process
     ├── Networking
     └── Actors
     │
     ▼
Runact Runtime
     │
     ├── Scheduler
     ├── Async Tasks
     ├── Actors
     ├── Supervision
     ├── Timers
     ├── Cancellation
     └── Compute
     │
     ▼
Operating System
```

---

# 3. Repository Structure

Use **one repository and one Cargo workspace**.

```text
runact/
│
├── Cargo.toml
├── README.md
├── LICENSE
│
├── crates/
│   │
│   ├── runact-core/
│   │
│   ├── runact-runtime/
│   │
│   ├── runact-supervision/
│   │
│   ├── runact-compute/
│   │
│   ├── runact-net/
│   │
│   ├── runact-process/
│   │
│   └── runact-web/
│
├── examples/
│   ├── actors/
│   ├── async/
│   ├── tcp-server/
│   ├── http-server/
│   ├── websocket-server/
│   └── ai-agent/
│
├── benchmarks/
│
└── tests/
```

The user-facing crate can eventually be:

```rust
use runact::Runtime;
```

and, when web functionality is enabled:

```rust
use runact::web::App;
```

The internal crates remain modular.

---

# 4. Layer 1 — runact-core

This is the fundamental actor model.

Responsibilities:

```text
Actor
ActorId
Message
Mailbox
ActorContext
Actor lifecycle
Actor errors
Basic handles
```

Do NOT put:

```text
HTTP
TCP
WebSocket
filesystem
processes
```

into `runact-core`.

The core should remain extremely small.

---

# 5. Actor Model

An actor owns its state.

Example:

```rust
struct Counter {
    value: u64,
}
```

Messages:

```rust
enum CounterMessage {
    Increment,
    Get,
}
```

Conceptually:

```text
Actor
 ├── State
 ├── Mailbox
 └── Lifecycle
```

Other actors communicate through messages rather than directly manipulating actor state.

Rust ownership should remain the primary safety mechanism.

Avoid making:

```rust
Arc<Mutex<GlobalState>>
```

the fundamental Runact architecture.

---

# 6. Lightweight Actors

Actors must be lightweight enough to support large numbers of concurrent actors.

Do NOT use:

```text
1 actor = 1 OS thread
```

Instead:

```text
Many Actors
     ↓
Scheduler
     ↓
Small execution slice
     ↓
Worker thread
     ↓
Yield
```

The exact number of actors Runact can support must be established through benchmarks.

---

# 7. Mailboxes

Every actor gets a mailbox.

Requirements:

* bounded
* FIFO by default
* efficient
* thread-safe
* wake-aware
* configurable capacity

Example:

```text
Actor
 │
 └── Mailbox(capacity)
```

When full:

```text
Sender
  ↓
Mailbox full
  ↓
Backpressure
```

Do not use unlimited queues as the default.

---

# 8. Scheduler

Implement a scheduler designed for high concurrency.

Initial architecture:

```text
Scheduler
 ├── Worker 1
 ├── Worker 2
 ├── Worker 3
 └── Worker N
```

Later:

```text
Worker-local queues
        +
Work stealing
        +
Global injection queue
```

Requirements:

* fairness
* low contention
* efficient wakeups
* work stealing
* actor execution budgets
* task scheduling
* no starvation

---

# 9. Cooperative Actor Execution

A single actor must not monopolize a worker.

Use an execution budget inspired by BEAM reductions.

Conceptually:

```text
Actor
 ↓
process messages
 ↓
consume budget
 ↓
budget exhausted
 ↓
yield
 ↓
scheduler
```

Do not copy BEAM's implementation literally.

Develop a Rust-native scheduling model.

---

# 10. Async Runtime

Runact must support standard Rust futures.

The runtime should provide:

```text
spawn()
sleep()
timeout()
cancel()
join()
task_group()
```

Conceptually:

```text
Runact
 ├── Actors
 ├── Async Tasks
 └── Compute Jobs
```

Use actors for stateful concurrent components.

Use async tasks for waiting/I/O.

Use compute jobs for CPU-heavy work.

---

# 11. Future Execution

Runact should be able to poll standard:

```rust
Future<Output = T>
```

without requiring every application to use Tokio.

Task lifecycle:

```text
Created
   ↓
Scheduled
   ↓
Running
   ↓
Waiting
   ↓
Runnable
   ↓
Running
   ↓
Completed
```

Failure:

```text
Running → Failed
```

Cancellation:

```text
Running/Waiting
      ↓
Cancelling
      ↓
Cancelled
```

---

# 12. Waker

Implement a Runact-specific Waker.

Flow:

```text
Future
  ↓
poll()
  ↓
Pending
  ↓
Waker registered
  ↓
I/O/timer/event occurs
  ↓
wake()
  ↓
Task marked Runnable
  ↓
Scheduler
```

Prevent duplicate scheduling using atomic task state.

---

# 13. Cancellation

Cancellation must be cooperative.

Provide:

```rust
CancellationToken
```

with operations such as:

```rust
cancel()
is_cancelled()
```

Use cancellation for:

* requests
* connections
* AI agents
* subprocesses
* background tasks
* shutdown

Never forcibly terminate arbitrary Rust code.

---

# 14. Structured Concurrency

Implement task groups.

Example:

```text
RequestTaskGroup
│
├── Database Task
├── API Task
├── File Task
└── Compute Task
```

If the request is cancelled:

```text
Request cancelled
       ↓
TaskGroup.cancel()
       ↓
Cancel children
```

This is important for the future web framework.

---

# 15. Timers

Implement centralized timers.

Required primitives:

```text
sleep()
timeout()
deadline()
```

Use a simple priority queue initially.

Later evaluate:

```text
timer wheel
```

only if benchmarks justify it.

---

# 16. Compute Pool

CPU-heavy work must not block actor or I/O workers.

Provide:

```rust
runtime.compute(|| {
    expensive_operation()
});
```

Architecture:

```text
Async/Actor Task
       ↓
Compute Job
       ↓
Compute Pool
       ↓
Result
       ↓
Task/Actor
```

Examples:

* parsing
* compression
* hashing
* indexing
* search
* code analysis
* AI preprocessing
* image processing

---

# 17. Fault Isolation

Runact must isolate actor failures.

Example:

```text
Supervisor
│
├── Actor A
├── Actor B
├── Actor C ← panic
└── Actor D
```

Desired behavior:

```text
Actor C panic
      ↓
Catch failure
      ↓
Actor C marked failed
      ↓
Resources cleaned up
      ↓
Supervisor notified
      ↓
Restart / Stop / Escalate
```

A single actor panic should not automatically terminate the entire runtime.

---

# 18. Supervision

Build supervision as a first-class Runact subsystem.

Example:

```text
ApplicationSupervisor
│
├── NetworkSupervisor
├── AgentSupervisor
├── WorkspaceSupervisor
└── WorkerSupervisor
```

Supervisors own the lifecycle policy of their children.

---

# 19. Restart Strategies

Implement initially:

```text
one_for_one
```

Example:

```text
A
B ← fails
C

Only B restarts.
```

Later:

```text
one_for_all
rest_for_one
```

---

# 20. Restart Policies

Support:

```text
Never
OnFailure
Always
```

and limits:

```text
max_restarts
restart_window
backoff
```

Prevent:

```text
crash
restart
crash
restart
...
```

from becoming an infinite restart loop.

---

# 21. Monitoring

Actors should be able to monitor other actors.

Events:

```text
ActorStarted
ActorStopped
ActorFailed
ActorRestarted
ActorTerminated
```

Later add actor links.

---

# 22. Resource Lifecycle

Use Rust ownership and `Drop` heavily.

Example:

```text
Actor
 ├── TCP connection
 ├── file
 ├── temporary data
 └── other resources
```

When the actor terminates:

```text
Actor dropped
     ↓
Rust Drop
     ↓
Resources released
```

This gives Runact a strong combination:

```text
Actor isolation
+
Rust ownership
+
RAII resource cleanup
```

---

# 23. runact-net

Create the networking layer inside the Runact ecosystem.

Responsibilities:

```text
TcpListener
TcpStream
Readiness
Reactor
Async read
Async write
Timeout
Cancellation
Backpressure
```

Linux first:

```text
epoll
```

Later:

```text
kqueue
IOCP
```

Do not initially implement:

```text
HTTP
WebSocket
TLS
DNS
HTTP/2
HTTP/3
QUIC
```

---

# 24. Why TCP Belongs in Runact

Networking is fundamental runtime infrastructure.

Runact needs it for:

```text
Web servers
AI agents
Remote PaperOS
WebSocket services
Custom protocols
Distributed actors
```

Therefore:

```text
runact-net
```

is a natural Runact subsystem.

---

# 25. TCP API

Provide a simple async API:

```rust
let listener = TcpListener::bind("0.0.0.0:8080").await?;

loop {
    let stream = listener.accept().await?;

    runtime.spawn(handle_connection(stream));
}
```

The TCP layer should not care whether the bytes represent:

```text
HTTP
WebSocket
AI protocol
custom binary protocol
```

---

# 26. Network Backpressure

Network buffers must be bounded.

Example:

```text
Application
    ↓
Bounded Buffer
    ↓
TcpStream
    ↓
OS
```

If the client is slow:

```text
write()
 ↓
Pending
 ↓
task yields
```

Do not silently accumulate unlimited response data.

---

# 27. Connection Lifecycle

```text
Accepted
   ↓
Active
   ↓
Reading
   ↓
Processing
   ↓
Writing
   ↓
KeepAlive
   ↓
Closed
```

Failure:

```text
I/O error
   ↓
connection cleanup
```

Cancellation:

```text
Cancel
   ↓
close connection
   ↓
release resources
```

---

# 28. runact-process

Provide process management for AI agents and PaperOS.

Responsibilities:

```text
spawn
stdin
stdout
stderr
exit status
timeout
cancellation
termination
cleanup
```

Architecture:

```text
Runact
  ↓
Process Task
  ↓
Child Process
```

Prevent orphan processes during shutdown.

---

# 29. Integrated Web Layer

Now build:

```text
runact-web
```

**inside the same Runact repository/workspace.**

It is not a separate ecosystem.

Architecture:

```text
runact-web
     ↓
Runact
     ↓
runact-net
     ↓
OS
```

---

# 30. runact-web Responsibilities

`runact-web` owns:

```text
HTTP
WebSocket
Request
Response
Headers
Routing
Middleware
Extractors
Streaming
Cookies
Sessions
Static files
```

Runact owns:

```text
Tasks
Actors
Scheduling
TCP
I/O
Timers
Cancellation
Supervision
Compute
```

---

# 31. HTTP Architecture

A request should flow approximately as:

```text
TCP connection
      ↓
HTTP parser
      ↓
Request
      ↓
Router
      ↓
Middleware
      ↓
Handler
      ↓
Application logic
      ↓
Response
      ↓
HTTP encoder
      ↓
TCP
```

Runact provides the concurrency underneath.

---

# 32. Web Server Concurrency

Example:

```text
WebServer
│
├── Connection Task 1
├── Connection Task 2
├── Connection Task 3
├── Connection Task 4
└── Connection Task N
```

Waiting connections consume very little execution capacity.

Only runnable tasks use workers.

---

# 33. Request Cancellation

If the client disconnects:

```text
Client disconnect
      ↓
Connection cancelled
      ↓
Request TaskGroup cancelled
      ↓
Database/API/File tasks cancelled
      ↓
Resources released
```

This is one of the major reasons Runact's cancellation system must be designed before `runact-web`.

---

# 34. HTTP Handler Model

The eventual API could look like:

```rust
let app = App::new()
    .route("/", get(index))
    .route("/users", get(users))
    .route("/users", post(create_user));

app.listen("0.0.0.0:8080").await?;
```

Do not lock the API too early.

First build the runtime underneath it.

---

# 35. WebSocket

Add WebSocket above TCP:

```text
runact-web
    ↓
WebSocket protocol
    ↓
runact-net
    ↓
Runact reactor
```

WebSocket connections can be represented by:

```text
Connection Task
+
Actor
```

depending on the application's needs.

---

# 36. Middleware

Middleware belongs in `runact-web`.

Examples:

```text
Logging
Authentication
Authorization
Compression
CORS
Rate limiting
Request timeout
Tracing
```

Runact should provide the primitives needed to implement these efficiently, but should not implement web-specific middleware in the runtime.

---

# 37. Streaming

The runtime must support efficient streaming.

Examples:

```text
HTTP streaming
Server-Sent Events
WebSocket streaming
AI token streaming
File downloads
```

Architecture:

```text
Producer
   ↓
Bounded stream
   ↓
Network task
   ↓
TcpStream
```

Backpressure must propagate toward the producer.

---

# 38. AI Agent Server

One of the first serious applications of Runact should be an AI agent server.

Architecture:

```text
Client
  │
  │ HTTP/WebSocket
  ▼
runact-web
  │
  ▼
AgentSupervisor
  │
  └── AgentActor
       │
       ├── LLM Task
       ├── File Task
       ├── Shell Task
       ├── Git Task
       └── Compute Job
```

Runact provides the concurrency and lifecycle management.

---

# 39. PaperOS

PaperOS should eventually use Runact like:

```text
PaperOS
   ↓
Runact
   ↓
Actors
Tasks
Supervision
Networking
Process management
```

Possible actors:

```text
WorkspaceActor
BufferActor
TerminalActor
GitActor
LspActor
AgentActor
ExtensionActor
```

A failing subsystem can be isolated.

---

# 40. Remote PaperOS Agent

Future architecture:

```text
PaperOS Desktop
       │
       │ HTTPS / WebSocket
       ▼
runact-web
       │
       ▼
AgentSupervisor
       │
       ▼
AgentActor
       │
       ├── LLM
       ├── Filesystem
       ├── Shell
       ├── Git
       └── Compute
```

This makes Runact useful beyond traditional web applications.

---

# 41. Security Boundary

Runact should eventually support capabilities.

Examples:

```text
filesystem.read
filesystem.write
network
process.spawn
shell
secrets
```

An AI agent should receive only explicitly granted capabilities.

Example:

```text
Agent
 ├── filesystem.read     ✓
 ├── filesystem.write    ✓
 ├── process.spawn       approval
 ├── network              ✗
 └── secrets              ✗
```

This belongs partly in the application/framework layer, but Runact should provide the lifecycle/resource primitives needed to enforce it.

---

# 42. Observability

Expose runtime metrics:

```text
Actor count
Task count
Runnable tasks
Worker utilization
Mailbox depth
Queue depth
Active connections
Bytes read
Bytes written
Task latency
I/O latency
Actor failures
Restart count
Compute queue depth
```

Do not force one particular observability stack.

Provide instrumentation hooks.

---

# 43. Graceful Shutdown

Runact must support:

```text
Runtime::shutdown()
Runtime::shutdown_with_timeout()
```

Shutdown:

```text
Stop accepting new work
        ↓
Stop accepting new connections
        ↓
Cancel task groups
        ↓
Allow active work to finish
        ↓
Stop actors
        ↓
Close connections
        ↓
Stop async workers
        ↓
Stop compute workers
        ↓
Release resources
```

The runtime must not wait forever.

---

# 44. Testing Strategy

Before building a serious web framework, test Runact itself.

## Actor tests

```text
message delivery
mailbox capacity
actor lifecycle
panic isolation
restart
monitoring
```

## Scheduler tests

```text
fairness
work stealing
starvation
queue contention
```

## Async tests

```text
Future polling
Waker
timeouts
cancellation
task groups
```

## Networking tests

```text
TCP connection
read/write
disconnect
slow client
backpressure
timeouts
cancellation
```

## Process tests

```text
spawn
stdout
stderr
exit
timeout
cancellation
cleanup
```

---

# 45. Fault Injection

Deliberately introduce failures:

```text
Actor panic
Task failure
Connection failure
Client disconnect
Mailbox overflow
Compute queue saturation
Process crash
Timeout
Cancellation
Runtime shutdown
```

Verify:

```text
Unrelated components continue
Resources are released
Supervisor reacts
Restart policy works
Runtime remains consistent
```

---

# 46. Concurrency Benchmark

Build a raw TCP benchmark before HTTP.

Test:

```text
100 connections
1,000 connections
10,000 connections
100,000 connections
```

Measure actual:

```text
Memory
CPU
Latency
Throughput
Wakeup latency
Scheduler latency
```

Do not make performance claims without measurements.

---

# 47. Web Benchmark

After `runact-net` is stable:

```text
runact-web
    ↓
HTTP benchmark
```

Measure:

```text
Requests/sec
P50 latency
P95 latency
P99 latency
Memory per connection
CPU utilization
Concurrent connections
```

Compare against established Rust frameworks/runtimes where useful.

The goal is measurement, not marketing claims.

---

# 48. Development Phases

## Phase 1 — Core

Build:

```text
runact-core
```

Implement:

```text
Actor
Message
Mailbox
ActorId
Lifecycle
```

---

## Phase 2 — Scheduler

Build:

```text
runact-runtime
```

Implement:

```text
Workers
Scheduler
Fairness
Execution budget
Work stealing
```

---

## Phase 3 — Async

Implement:

```text
Future
Task
Waker
Cancellation
Timers
TaskGroup
```

---

## Phase 4 — Compute

Implement:

```text
ComputePool
ComputeJob
Backpressure
```

---

## Phase 5 — Fault Tolerance

Implement:

```text
Supervisor
RestartPolicy
Monitoring
Backoff
Failure isolation
```

---

## Phase 6 — Networking

Implement:

```text
runact-net
TCP
epoll
Readiness
Async read/write
Network cancellation
Timeouts
Backpressure
```

---

## Phase 7 — Process Management

Implement:

```text
runact-process
```

---

## Phase 8 — Runtime Validation

Build:

```text
TCP echo server
TCP benchmark server
fault injection tests
concurrency tests
shutdown tests
```

Do not start HTTP until these are stable.

---

## Phase 9 — HTTP

Build:

```text
runact-web
```

Start with:

```text
HTTP/1.1
Request
Response
Headers
Parser
Encoder
```

---

## Phase 10 — Web Framework

Add:

```text
Router
Handlers
Middleware
Extractors
Streaming
Static files
```

---

## Phase 11 — WebSocket

Add:

```text
Upgrade
Frames
Connection lifecycle
Streaming
Backpressure
Cancellation
```

---

## Phase 12 — Real Applications

Build:

```text
REST API
WebSocket server
AI agent server
Remote PaperOS server
```

---

# 49. What NOT to Build in Runact Core

Do not put these into the runtime core:

```text
HTTP
WebSocket
HTML
Templates
REST
GraphQL
Authentication
Cookies
Sessions
ORM
Database abstractions
Business logic
```

They can exist in first-party Runact ecosystem crates.

For example:

```text
runact-web
runact-auth
runact-db
```

but they should remain above the runtime.

---

# 50. Long-Term Ecosystem

Eventually the Runact ecosystem can look like:

```text
                         Runact Ecosystem
                                │
       ┌────────────────────────┼────────────────────────┐
       │                        │                        │
    runact-web              PaperOS                 AI Agents
       │                        │                        │
       │                        │                        │
       └────────────────────────┼────────────────────────┘
                                │
                             Runact
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
      Actors                Async Runtime          Compute
        │                       │                       │
        ├──────────────┐        │                       │
        │              │        │                       │
 Supervision        Mailbox   Timers                  Jobs
        │              │        │                       │
        └──────────────┴────────┴───────────────────────┘
                                │
                           runact-net
                                │
                             TCP/I/O
                                │
                                ▼
                               OS
```

---

# 51. The Critical Boundary

Keep this boundary stable:

```text
┌─────────────────────────────────────┐
│           APPLICATIONS              │
│ PaperOS / AI / Web Applications     │
├─────────────────────────────────────┤
│          RUNACT-WEB                  │
│ HTTP / WebSocket / Routing           │
├─────────────────────────────────────┤
│             RUNACT                   │
│ Actors / Tasks / Scheduler           │
│ Supervision / Timers / Cancellation  │
│ Compute / Process / Networking       │
├─────────────────────────────────────┤
│               OS                     │
└─────────────────────────────────────┘
```

The **repository is unified**, but the **architecture is layered**.

---

# 52. Final Principle

Do not think:

> "Runact and runact-web are two different projects."

Think:

> **Runact is the platform. `runact-web` is Runact's web layer.**

Use:

```text
runact
runact-web
runact-net
runact-process
runact-compute
```

as modular crates inside one ecosystem.

The runtime should remain useful without HTTP, while HTTP should be able to exploit every important Runact capability.

---

# 53. Final Goal

The final developer experience should eventually be something like:

```rust
use runact::Runtime;
use runact::web::{App, get};

async fn index() -> &'static str {
    "Hello from Runact"
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new();

    runtime.block_on(async {
        let app = App::new()
            .route("/", get(index));

        app.listen("0.0.0.0:8080").await
    })?;

    Ok(())
}
```

The application developer sees:

```text
Runact
```

not a collection of unrelated runtimes.

Underneath, however:

```text
App
 ↓
HTTP
 ↓
TCP
 ↓
Runact async task
 ↓
Waker
 ↓
Reactor
 ↓
Scheduler
 ↓
Worker
 ↓
OS
```

This is the architecture Runact should be designed toward from the beginning.
