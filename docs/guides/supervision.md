# Supervision

## Overview

Supervisors manage child actor lifecycles and handle failures. When a child crashes, the supervisor decides whether to restart it based on its strategy.

## Creating a Supervisor

```rust
use runact::{Runtime, RestartStrategy, ChildSpec, RestartPolicy};
use std::time::Duration;

let mut runtime = Runtime::new().unwrap();

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

### OneForOne

Only the failed child is restarted. Siblings are unaffected.

```rust
RestartStrategy::OneForOne {
    max_restarts: 5,        // Max restarts before giving up
    within: Duration::from_secs(60),  // Time window for counting
    base_backoff: Duration::from_millis(100),  // Initial delay
}
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

Backoff doubles each restart (via bit shift) and is capped at 30 seconds. If the backoff period has not elapsed since the last restart, the child stays dead.

## Restart Policies

| Policy | Behavior |
|--------|----------|
| `Permanent` | Always restart, regardless of exit reason |
| `Temporary` | Never restart |
| `Transient` | Restart only on abnormal exit (crash, error) |

## Failure Handling

### What is a Failure?

- Compute task panic (caught by compute worker, surfaced as `ComputeError::WorkerPanic`)
- Actor handler returns `Err(ActorError)`

### What is NOT a Failure?

- Actor returns `Ok(())`
- Compute task completes successfully
- Actor stops gracefully (runtime shutdown)

### Failure Flow

```text
Child crashes
    │
    ▼
Supervisor notified (via SupervisorMessage::ChildCrashed)
    │
    ├── Check restart count vs max_restarts
    ├── Check backoff elapsed
    │
    ├── Restart → child is alive again
    │
    └── Give up → child stays dead (max_restarts exceeded)
```

## Supervisor Status

You can query supervisor status:

```rust
let handle = runtime.request(supervisor_id, SupervisorMessage::GetStatus).unwrap();
let reply = handle.recv_timeout(Duration::from_secs(1)).unwrap();
let statuses = reply.downcast::<Vec<ChildStatus>>().unwrap();

for status in &statuses {
    println!("{}: alive={}, restarts={}", status.name, status.alive, status.restart_count);
}
```

## Best Practices

1. **Start simple** — OneForOne is usually sufficient
2. **Set limits** — Always configure max_restarts to prevent infinite restart loops
3. **Use backoff** — Give failing actors time to recover via base_backoff
4. **Log failures** — All restart events are logged via tracing
5. **Design for failure** — What happens if actor panics?
6. **Test supervision** — Inject failures deliberately via `ChildCrashed` messages
7. **Choose Transient for stateless workers** — Only restart on crashes, not clean exits
8. **Choose Permanent for critical services** — Always restart regardless of exit reason
