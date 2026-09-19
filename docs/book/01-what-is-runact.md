# Chapter 1: What Is Runact?

## The Actor Model

The **actor model** is a way to structure concurrent computation. It was
originally developed by Carl Hewitt in 1973 and later popularized by Erlang
(the BEAM virtual machine). In the actor model:

- **Every actor is an independent unit of computation.** It has its own
  state, isolated from all other actors.
- **Actors communicate exclusively by passing messages.** You never touch
  another actor's memory directly.
- **An actor decides what to do with each message:** send messages to other
  actors, spawn new actors, change its own behavior (state), or schedule a
  future action.

This gives you **location transparency** (an actor doesn't need to know
whether its message target runs on the same machine or a remote one) and
**fault isolation** (a crash in one actor doesn't corrupt another actor's
memory).

## Why Rust?

Rust brings two key advantages to the actor model:

1. **Ownership guarantees**: Because actors never share mutable memory, Rust's
   borrow checker aligns perfectly with actor isolation. Messages are moved
   (not shared), so the compiler can prove at compile time that no aliasing
   or data races exist.

2. **Zero-cost abstractions**: Runact's actors and schedulers compile down to
   efficient machine code. There is no garbage collector, no VM overhead.

## BEAM Inspiration

Runact's scheduler is inspired by the BEAM (Erlang VM) scheduler:

```
                    ┌───────────────┐
                    │   Scheduler   │
                    │  (work-stealing)│
                    └────┬───┬───┬───┘
                         │   │   │
                    ┌────┘   │   └────┐
                 ┌──▼──┐  ┌──▼──┐  ┌─▼──┐
                 │ P1  │  │ P2  │  │PN  │  ← Worker threads
                 │     │  │     │  │    │  ← Run queues
                 └─────┘  └─────┘  └────┘
```

Each worker thread maintains a run queue. When a worker runs out of work,
it **steals** work from another worker's queue. Actors (called "processes"
in BEAM terms) are extremely lightweight — a single Runact actor can cost
as little as a few hundred bytes of heap.

## Runact at a Glance

| Concept          | Runact Type              | Purpose                                    |
|------------------|--------------------------|--------------------------------------------|
| Actor            | `impl Actor`             | Isolated unit of state + behavior         |
| Message          | `Actor::Message`         | Data sent between actors                  |
| ActorId          | `ActorId`                | Unique address of an actor                |
| Runtime          | `Runtime`                | Top-level coordinator                     |
| Scheduler        | `Scheduler` (private)    | Assigns actors to worker threads          |
| Supervision      | `Supervisor`             | Fault-tolerant child management           |
| Async tasks      | `spawn`, `TaskHandle`    | Standard Rust `Future` execution          |
| Compute pool     | `ComputeHandle`          | CPU-bound work off the scheduler          |
| Timers           | `schedule_timer`         | Delayed and periodic message delivery     |
| TCP              | `TcpListener`, `TcpStream`| Native async I/O readiness               |

## Design Principles

1. **Runact owns concurrency and task lifecycle.** It manages actors,
   async tasks, the scheduler, and I/O readiness.

2. **Higher-level libraries own protocols.** HTTP, WebSocket, TLS, DNS, and
   application-level protocols are implemented in separate crates
   (`runact-web` for HTTP/WebSocket). Runact core has no these dependencies.

3. **No Tokio in the core.** Runact implements its own executor. Tokio
   integration is possible at the application layer if needed, but it is
   not fundamental to Runact's operation.

4. **Actors are synchronous and cooperative.** An actor's `handle` method
   runs to completion for each message. It must return quickly — no blocking.

5. **CPU-heavy work never runs on actor or async workers.** It goes to the
   dedicated compute pool.

6. **Cancellation is cooperative.** Tasks and actors can be told to stop,
   but they must check for cancellation themselves.

## Quick Start

Here is the complete "Hello, World" in Runact:

```rust
use runact::{Actor, ActorContext, ActorError, Runtime};

struct Greeter;

impl Actor for Greeter {
    type Message = String;

    fn handle(
        &mut self,
        msg: String,
        ctx: &mut ActorContext,
    ) -> Result<(), ActorError> {
        println!("Hello, {}!", msg);
        Ok(())
    }
}

fn main() {
    let mut runtime = Runtime::new().unwrap();
    let id = runtime.spawn(Greeter).unwrap();
    runtime.send(id, "world".to_string()).unwrap();
    runtime.shutdown();
}
```

This creates a runtime, spawns one actor, sends it a message, and shuts down.
Chapter 3 walks through this example line by line.
