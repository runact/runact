# Runact Vision

## What Runact Is

Runact is a Rust-native concurrent runtime and application platform inspired by BEAM.

It provides lightweight actors, message passing, supervision, timers, async tasks, compute pools, process management, and TCP networking.

Runact is a **platform**, not an application. Applications are built on top of it.

> **Runact is one ecosystem, with multiple layers and crates.**

## What Runact Is NOT

- Not an editor
- Not an IDE
- Not a BEAM clone
- Not a distributed system (initially)
- Not a custom programming language
- Not an HTTP/WebSocket framework (these are layered on top)

## Core Purpose

> Build a small, reliable, extensible runtime where software components can participate through stable APIs.

## Design Priorities

1. **Correctness** — Behavior must match specification. No silent failures.
2. **Simplicity** — Every component must be understandable in isolation.
3. **Clear boundaries** — Public APIs must not leak implementation details.
4. **Testability** — Every component must be testable without external dependencies.
5. **Failure isolation** — One component's crash must not bring down the system.
6. **Stable APIs** — Public interfaces must survive internal refactors.
7. **Observability** — Every important operation must be traceable.
8. **Security** — Capabilities must be explicit. No implicit trust.
9. **Performance** — Measure before optimizing. No premature optimization.
10. **Long-term maintainability** — Code must be readable in 10 years.

## Architectural Principle

**The runtime is the foundation.**

Applications are clients of the runtime. Examples:

- TUI applications
- GUI applications
- CLI tools
- Web servers
- AI agent systems
- Automation workflows
- Remote services
- PaperOS

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

## What Runact Provides

- Lightweight actors with stable `ActorId` identities
- Typed message passing with ownership transfer
- Bounded mailboxes with explicit backpressure (`MailboxFull` error)
- Supervision trees with `OneForOne` restart strategy and exponential backoff
- Timers (one-shot and periodic with drift correction)
- Cancellation (cooperative, via `CancellationToken`)
- Structured concurrency (via `TaskGroup`)
- Async task execution (standard Rust `Future`s on a native Runact executor)
- Dedicated compute pool for CPU-intensive work with panic isolation
- Process management (spawn, stdin/stdout/stderr, exit status, timeout, cancellation)
- TCP networking (reactor, `TcpListener`, `TcpStream`)
- Capability-based resource management (`Capability`, `ResourceHandle`, `ResourceRegistry`)
- Observability via `tracing` (lifecycle events, restarts, crashes)
- Runtime statistics (`RuntimeStats`, `ActorInfo`)

## What Runact Does NOT Provide

- Text editing
- Buffer management
- Cursor/selection handling
- LSP integration
- Git integration
- Terminal emulation
- UI rendering
- HTTP/WebSocket protocol implementations (these are layered on top via `runact-web`)
- Database abstractions
- Authentication
- Business logic

These are **application concerns**, not runtime concerns.

An application built on Runact would implement these as actors or in higher-level crates (`runact-web`, `runact-auth`, etc.).

## Success Criteria

Runact v1 is complete when:

- Process failures are isolated (compute task panics caught)
- Supervisors reliably restart services (via `SupervisorActor`)
- Thousands of actors run efficiently (benchmarks: 100K actors)
- Message passing is fast and reliable (request-reply latency benchmarks)
- Timers fire correctly (one-shot and periodic)
- Cancellation works (`CancellationToken`, `TaskGroup`)
- Async tasks execute standard Rust futures
- Process management works (spawn, stdin/stdout/stderr, exit status)
- TCP networking works (reactor, `TcpListener`, `TcpStream`)
- Shutdown is graceful (`Runtime::shutdown` with configurable timeout)
- APIs are stable and documented (all public types have rustdoc)
- Runtime internals are not leaked (no `crossbeam` types in public APIs)

## Long-Term Evolution

```text
Runtime Core
    ↓
Reliability (Supervision)
    ↓
Compute Pool
    ↓
Timers and Cancellation
    ↓
Resources and Capabilities
    ↓
Async Runtime (Future Executor, Task Groups)
    ↓
Process Runtime
    ↓
TCP Networking (Reactor, TcpListener, TcpStream)
    ↓
HTTP (runact-web)
    ↓
Web Framework (Router, Middleware, Extractors)
    ↓
WebSocket
    ↓
Real Applications (REST API, AI Agent Server, PaperOS)
```

The runtime must earn every layer of complexity.

See [Development Plan](development-plan.md) for the full 53-section architecture document.

## The Key Insight

> What is the smallest runtime that makes building concurrent systems fundamentally better?

Build that runtime. Then let real applications determine evolution.

The long-term asset is not features. It is the stability of the underlying model:

- Actors (processes)
- Messages
- Supervision
- Timers
- Capabilities
- Async Tasks
- Cancellation
- Structured Concurrency
- Process Management
- TCP Networking
- Observability

Everything else should remain replaceable.
