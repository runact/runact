# Supervision

## Overview

Runact provides BEAM-inspired supervision. Supervisors manage actor lifecycle and handle failures. When a child crashes, the supervisor decides whether to restart it based on its strategy.

## Core Principle

> **Let it crash — but supervise the crash.**

## Supervision Tree

```text
WebAppSupervisor
│
├── HttpSupervisor
│   ├── ConnectionHandler
│   ├── ConnectionHandler
│   └── ConnectionHandler
│
├── DbSupervisor
│   ├── PoolManager
│   └── QueryExecutor
│
├── CacheSupervisor
│   └── CacheActor
│
└── WorkerSupervisor
    ├── BackgroundJob
    └── BackgroundJob
```

## Creating a Supervisor

```rust
use runact::{Runtime, RestartStrategy, ChildSpec, RestartPolicy};

let strategy = RestartStrategy::OneForOne {
    max_restarts: 5,
    within: Duration::from_secs(60),
    base_backoff: Duration::from_millis(100),
};

let children = vec![
    ChildSpec::new("worker-1")
        .restart_policy(RestartPolicy::Permanent),
    ChildSpec::new("worker-2")
        .restart_policy(RestartPolicy::Transient)
        .shutdown_timeout(Duration::from_secs(2)),
];

let supervisor_id = runtime.spawn_supervisor(strategy, children).unwrap();
```

## Restart Strategies

### One-for-One (Default and Only Supported Strategy)

```text
Child crashes → Supervisor restarts that child
```

```rust
pub enum RestartStrategy {
    OneForOne {
        max_restarts: usize,
        within: Duration,
        base_backoff: Duration,
    },
}
```

Only the failed child is restarted. Siblings are unaffected.

### One-for-All and Rest-for-One

Planned for future releases:

```text
One-for-All: Child crashes → All children restart
Rest-for-One: Child crashes → That child and all children started after it restart
```

## Child Specification

```rust
pub struct ChildSpec {
    pub name: String,
    pub restart_policy: RestartPolicy,
    pub shutdown_timeout: Duration,
}

pub enum RestartPolicy {
    Permanent,   // Always restart
    Temporary,   // Never restart
    Transient,   // Restart only on abnormal exit
}
```

### Builder Pattern

```rust
ChildSpec::new("my-worker")
    .restart_policy(RestartPolicy::Permanent)
    .shutdown_timeout(Duration::from_secs(5))
```

## Exponential Backoff

When a child crashes repeatedly, the supervisor applies exponential backoff:

```text
restart 1: 100ms
restart 2: 200ms
restart 3: 400ms
restart 4: 800ms
restart 5: 1600ms
...
```

Backoff doubles each restart (via bit shift) and is capped at 30 seconds.

The backoff is computed as:

```rust
fn backoff_delay(&self, restart_count: usize) -> Duration {
    let base = self.base_backoff();
    let shift = (restart_count as u32).saturating_sub(1).min(30);
    let multiplier = 1u32 << shift;
    let delay = base * multiplier;
    delay.min(Duration::from_secs(30))
}
```

If the backoff period has not elapsed since the last restart, the child stays dead and the supervisor logs a warning.

## Failure Handling

### What is a Failure?

- Compute task panic (caught by compute worker, surfaced as `ComputeResult::Panic`)
- Actor handler returns `Err(ActorError)`

### What is NOT a Failure?

- Actor returns `Ok(())`
- Actor stops gracefully (runtime shutdown)
- Compute task completes successfully

### Failure Flow

```text
Actor/Compute Task fails
    │
    ▼
Supervisor notified (via SupervisorMessage::ChildCrashed)
    │
    ├── Check restart count vs max_restarts
    ├── Check backoff elapsed
    │
    ├── Restart → Restart actor (within limits)
    │
    ├── Stop → Mark child as dead (if backoff not elapsed or max exceeded)
    │
    └── Escalate → Log error, child stays dead
```

### Restart Limits

The `max_restarts` field limits restarts within the `within` time window. When exceeded, the supervisor logs an error and the child is not restarted:

```rust
tracing::error!(
    child_id = %child_id,
    restart_count = new_restart_count,
    max_restarts = max_restarts,
    reason = %reason,
    "child exceeded max restarts, giving up"
);
```

## Supervisor Message Protocol

The `SupervisorActor` implements the `Actor` trait and accepts:

```rust
pub enum SupervisorMessage {
    ChildStarted { child_id: ActorId, name: String },
    ChildCrashed { child_id: ActorId, reason: String },
    GetStatus,
}
```

You can query supervisor status by sending a `GetStatus` request:

```rust
let handle = runtime.request(supervisor_id, SupervisorMessage::GetStatus)?;
let reply = handle.recv_timeout(Duration::from_secs(1))?;
let statuses = reply.downcast::<Vec<ChildStatus>>()?;
```

### ChildStatus

```rust
pub struct ChildStatus {
    pub id: ActorId,
    pub name: String,
    pub restart_count: usize,
    pub alive: bool,
}
```

## Crash Reporting

When a child crashes, the supervisor logs:

1. The crash event with child ID and reason
2. The restart count
3. The backoff delay
4. The decision (restart or give up)

```rust
tracing::warn!(
    child_id = %child_id,
    name = %entry.name,
    restart_count = new_restart_count,
    max_restarts = max_restarts,
    backoff_ms = delay.as_millis() as u64,
    reason = %reason,
    "child crashed, restarting"
);
```

## Monitoring vs Supervision

### Supervision

- Ownership relationship
- Supervisor manages child lifecycle
- Can restart, stop, or escalate

### Monitoring

- Observation relationship
- Observer notified of events
- Cannot affect observed actor

Monitoring is not yet implemented but is planned for future releases.

## Best Practices

1. **Start simple** — One-for-one is usually sufficient
2. **Set limits** — Always configure `max_restarts` to prevent infinite restart loops
3. **Use backoff** — Give failing actors time to recover via `base_backoff`
4. **Log failures** — All restart events are logged via `tracing`
5. **Design for failure** — What happens if actor panics?
6. **Test supervision** — Inject failures deliberately via `ChildCrashed` messages
7. **Use `Transient` for clean-exit actors** — Only restart on crashes, not graceful stops
8. **Use `Permanent` for critical services** — Always restart regardless of exit reason
9. **Keep child specs declarative** — Use the builder pattern for clarity
10. **Monitor via tracing** — All lifecycle events are observable
