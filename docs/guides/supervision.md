# Supervision

## Overview

Supervisors manage child actor lifecycles and handle failures. When a child crashes, the supervisor decides whether to restart it based on its strategy.

## Creating a Supervisor

```rust
use runact::{Runtime, Supervisor, RestartStrategy, ChildSpec, RestartPolicy};
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
    max_restarts: 5,        // Max restarts before escalating
    within: Duration::from_secs(60),  // Time window for counting
    base_backoff: Duration::from_millis(100),  // Initial delay
}
```

## Exponential Backoff

When a child crashes repeatedly, the supervisor applies exponential backoff:

```
restart 1: 100ms
restart 2: 200ms
restart 3: 400ms
restart 4: 800ms
restart 5: 1600ms
...
```

Backoff doubles each restart and is capped at 30 seconds.

## Restart Policies

| Policy | Behavior |
|--------|----------|
| `Permanent` | Always restart, regardless of exit reason |
| `Temporary` | Never restart |
| `Transient` | Restart only on abnormal exit (panic, error) |

## Failure Handling

When `max_restarts` is exceeded, the supervisor escalates:

```rust
use tracing::{info, warn, error};

// The supervisorActor logs:
// warn!(child_id, restart_count, "Restarting actor")
// error!(child_id, restart_count, "Too many restarts, escalating")
```

## Best Practices

1. **Set `max_restarts`** — Prevent infinite restart loops
2. **Use `base_backoff`** — Give failing actors time to recover
3. **Choose `Transient` for stateless workers** — Only restart on crashes, not clean exits
4. **Monitor via `tracing`** — All restart events are logged
