# Chapter 7: Supervision

Supervision is Runact's fault-tolerance mechanism. A `Supervisor` manages
a tree of child actors, restarting them when they crash according to a
configurable `RestartStrategy`.

---

## 7.1 Why Supervision?

In distributed and long-running systems, failures are inevitable. Rather
than letting a single actor crash bring down your entire application,
supervision isolates failures and recovers automatically.

The Erlang/BEAM philosophy is: **let it crash**. Instead of defensive
programming everywhere, you write your actors to fail fast on errors,
and let the supervisor restart them in a known-good state.

## 7.2 RestartStrategy

```rust
pub enum RestartStrategy {
    OneForOne {
        max_restarts: usize,
        within: Duration,
        base_backoff: Duration,
    },
}
```

### OneForOne

When a child actor fails, **only that child is restarted**. Siblings are
unaffected. This is the simplest and most common strategy.

### Parameters

| Field | Default | Description |
|-------|---------|-------------|
| `max_restarts` | 5 | Maximum restarts within the `within` window before giving up |
| `within` | 60s | Time window for counting restarts |
| `base_backoff` | 100ms | Initial delay between restarts; doubles each time, capped at 30s |

If a child crashes more than `max_restarts` times within `within`, the
supervisor **escalates** — it fails itself, and its own supervisor
(if any) must handle the failure.

### Default Strategy

```rust
let strategy = RestartStrategy::default();
// OneForOne { max_restarts: 5, within: 60s, base_backoff: 100ms }
```

## 7.3 ChildSpec

A `ChildSpec` describes a child actor for the supervisor:

```rust
pub struct ChildSpec {
    pub name: String,
    pub restart_policy: RestartPolicy,
    pub shutdown_timeout: Duration,
}
```

### RestartPolicy

| Policy | Behavior |
|--------|----------|
| `Permanent` | Always restart on failure |
| `Temporary` | Never restart |
| `Transient` | Restart only on abnormal exit (panic, not clean error) |

### Builder Pattern

```rust
let child = ChildSpec::new("worker")
    .restart_policy(RestartPolicy::Permanent)
    .shutdown_timeout(Duration::from_secs(10));
```

## 7.4 Spawning a Supervisor

```rust
use runact::{
    ChildSpec, RestartPolicy, RestartStrategy, Runtime, Supervisor,
};

let mut runtime = Runtime::new().unwrap();

let strategy = RestartStrategy::default();
let children = vec![
    ChildSpec::new("worker-1"),
    ChildSpec::new("worker-2"),
];

let supervisor_id = runtime.spawn_supervisor(strategy, children).unwrap();
```

> **Note:** `spawn_supervisor` creates the supervisor as a registered
> actor within the runtime. The supervisor's `ActorId` is returned so you
> can send it messages if needed. In the current implementation, the
> supervisor tracks child metadata; actual child actor spawning follows
> the patterns in Chapter 16 (Agent Servers).

## 7.5 How Restarts Work

```
Child panics in handle()
    │
    ▼
Supervisor detects failure
    │
    ▼
Check restart budget: count < max_restarts within window?
    ├── Yes → wait base_backoff (×2^count), then restart child
    └── No  → escalate (fail supervisor)
```

### Exponential Backoff

```
Restart 1: 100ms
Restart 2: 200ms
Restart 3: 400ms
Restart 4: 800ms
...capped at 30s
```

This prevents rapid crash loops from overwhelming the system. If a child
has a persistent bug, exponential backoff gives you time to notice and
intervene before the system is consumed.

## 7.6 Supervised Worker Example

```rust
use runact::{Actor, ActorContext, ActorError};

struct Worker {
    config: WorkerConfig,
}

impl Actor for Worker {
    type Message = WorkItem;

    fn handle(&mut self, msg: WorkItem, ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        // If this panics, the supervisor will restart us.
        // Our state will be re-initialized as a fresh Worker.
        let result = process_item(&self.config, &msg);
        result.map_err(|e| ActorError::Handler(e.to_string()))
    }
}
```

When the supervisor restarts `Worker`, a new instance is created with
fresh state. The old state (including any corrupted or inconsistent
fields) is gone.

## 7.7 Supervisor as an Actor

In Runact's current implementation, `Supervisor` is not itself an `Actor`
trait implementor — it is managed by the runtime's internal supervision
machinery. The `Supervisor` struct tracks child specs and restart counts
and is used by `spawn_supervisor`. Future versions will expose the
supervisor as a first-class actor that can receive `HealthCheck` messages
and dynamic child specifications.

## 7.8 Key Principles

1. **Isolation**: A crash in one child doesn't affect siblings.
2. **Recovery**: Automatic restart with clean state.
3. **Rate limiting**: Too-fast restarts trigger backoff or escalation.
4. **Let it crash**: Don't catch every error — let the supervisor handle it.
