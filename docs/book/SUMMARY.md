# The Runact Runtime — A Reference Manual

> Runact is a Rust-native actor runtime that combines BEAM-style lightweight
> processes, messaging, scheduling, and supervision with Rust's ownership
> model. This book is a comprehensive reference for developers building
> concurrent and distributed systems with Runact.

**Audience**: Beginner to intermediate Rust developers who want to build
concurrent applications with actors, async tasks, and compute pools.

## Contents

### Part I: Foundations
1. [What Is Runact?](01-what-is-runact.md)
2. [Core Concepts](02-core-concepts.md)
3. [Your First Actor](03-first-actor.md)

### Part II: The Actor System
4. [The Actor Trait](04-actor-trait.md)
5. [Actor Context](05-actor-context.md)
6. [Mailboxes and Backpressure](06-mailboxes.md)
7. [Supervision](07-supervision.md)
8. [Actor-to-Actor Messaging](08-messaging.md)

### Part III: Async & Compute
9. [Async Tasks](09-async-tasks.md)
10. [Timers](10-timers.md)
11. [Compute Pool](11-compute.md)

### Part IV: I/O & Networking
12. [TCP Runtime](12-tcp.md)
13. [HTTP with runact-web](13-http.md)
14. [WebSocket Server](14-websocket.md)
15. [Message Fragmentation](15-fragmentation.md)

### Part V: Building Applications
16. [Agent Servers](16-agent-servers.md)
17. [REST APIs](17-rest-api.md)

### Part VI: Reference
18. [API Reference](18-api-reference.md)
19. [Invariants](19-invariants.md)
20. [Architectural Decisions](20-adrs.md)

---
