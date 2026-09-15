# Runact Vision

## What Runact Is

Runact is a minimal BEAM-inspired actor runtime built in Rust.

It provides lightweight processes, message passing, supervision, and timers.

Runact is a **runtime**, not an application. Applications are built on top of it.

## What Runact Is NOT

- Not an editor
- Not an IDE
- Not a BEAM clone
- Not a distributed system (initially)
- Not a custom programming language

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

```
                RUNACT RUNTIME
                      │
          ┌───────────┼───────────┐
          │           │           │
         TUI         GUI        Server
          │           │           │
          └───────────┼───────────┘
                      │
                Application
                      │
       ┌──────────────┼──────────────┐
       │              │              │
   Processes     Messages       Supervisors
       │              │              │
       └──────────────┼──────────────┘
                      │
                     Rust
```

## What Runact Provides

- Lightweight processes with stable identities
- Typed message passing
- Supervision trees with restart policies
- Timers and cancellation
- Capability-based security
- Observable operations

## What Runact Does NOT Provide

- Text editing
- Buffer management
- Cursor/selection handling
- LSP integration
- Git integration
- Terminal emulation
- UI rendering

These are **application concerns**, not runtime concerns.

An application built on Runact would implement these as processes.

## Success Criteria

Runact v1 is complete when:

- Process failures are isolated
- Supervisors reliably restart services
- Thousands of processes run efficiently
- Message passing is fast and reliable
- Timers fire correctly
- Cancellation works
- Shutdown is graceful
- APIs are stable and documented
- Runtime internals are not leaked

## Long-Term Evolution

```
minimal actor runtime
        ↓
workspace runtime
        ↓
programmable environment
        ↓
agent runtime
        ↓
optional VM
        ↓
optional distributed runtime
```

The runtime must earn every layer of complexity.

## The Key Insight

> What is the smallest runtime that makes building concurrent systems fundamentally better?

Build that runtime. Then let real applications determine evolution.

The long-term asset is not features. It is the stability of the underlying model:

- Processes
- Messages
- Supervision
- Timers
- Capabilities
- Observability

Everything else should remain replaceable.
