# Supervision

## Overview

Runact provides BEAM-inspired supervision. Supervisors manage actor lifecycle and handle failures.

## Core Principle

> **Let it crash — but supervise the crash.**

## Supervision Tree

```text
EditorSupervisor
│
├── UISupervisor
│   └── UIActor
│
├── BufferSupervisor
│   ├── BufferActor
│   ├── BufferActor
│   └── BufferActor
│
├── LspSupervisor
│   ├── RustAnalyzer
│   └── TypeScriptServer
│
└── TerminalSupervisor
    └── ShellActor
```

## Supervisor Trait

```rust
pub trait Supervisor: Actor {
    fn strategy(&self) -> RestartStrategy;
    fn children(&self) -> &[ChildSpec];
    fn handle_child_failure(&mut self, child_id: ActorId, reason: FailureReason);
}
```

## Restart Strategies

### One-for-One (Default)

```text
Child crashes → Supervisor restarts that child
```

- Other children unaffected
- Simplest strategy
- Good for independent children

### One-for-All

```text
Child crashes → All children restart
```

- Used when children depend on each
- Nuclear option
- Ensures consistency

### Rest-for-One

```text
Child crashes → That child and all children started after it restart
```

- Used for ordered initialization
- Middle ground between one-for-one and one-for-all

## Child Specification

```rust
pub struct ChildSpec {
    pub name: String,
    pub actor_type: ActorType,
    pub restart_policy: RestartPolicy,
    pub shutdown_timeout: Duration,
}

pub enum ActorType {
    Worker,  // Normal actor
    Supervisor,  // Another supervisor
}

pub enum RestartPolicy {
    Permanent,   // Always restart
    Temporary,   // Never restart
    Transient,   // Restart only on abnormal exit
}
```

## Failure Handling

### What is a Failure?

- Actor handler returns `Err(ActorError)`
- Actor panics
- Actor stops unexpectedly

### What is NOT a Failure?

- Actor returns `Ok(())`
- Actor stops gracefully
- Actor receives shutdown signal

### Failure Flow

```text
Actor fails
    │
    ▼
Supervisor notified
    │
    ▼
Supervisor checks strategy
    │
    ├── Restart → Restart actor
    │
    ├── Stop → Stop actor permanently
    │
    └── Escalate → Notify parent supervisor
```

## Backoff

Prevent infinite restart loops.

```rust
pub struct BackoffConfig {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub max_restarts: usize,
    pub within: Duration,  // Reset counter after this period
}
```

### Example

```rust
let supervisor = SupervisorBuilder::new()
    .child(ChildSpec::new("worker"))
    .backoff(BackoffConfig {
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(30),
        multiplier: 2.0,
        max_restarts: 5,
        within: Duration::from_secs(60),
    })
    .build();
```

## Crash Reporting

When a child crashes, the supervisor:

1. Logs the crash with full context
2. Updates restart counters
3. Applies backoff if needed
4. Restarts the child
5. Escalates if restart limit exceeded

```rust
tracing::error!(
    child_id = %child_id,
    child_name = %child_name,
    reason = %reason,
    restart_count = count,
    "Actor failed, restarting"
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

```rust
// Supervision
supervisor.spawn_child(child_spec);

// Monitoring
runtime.monitor(watcher_id, watched_id);
```

## Examples

### Simple Supervisor

```rust
struct BufferSupervisor {
    children: Vec<ChildSpec>,
    restart_counts: HashMap<ActorId, usize>,
}

impl Actor for BufferSupervisor {
    type Message = SupervisorMessage;

    fn handle(&mut self, msg: SupervisorMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            SupervisorMessage::ChildFailed { child_id, reason } => {
                self.handle_child_failure(child_id, reason);
            }
        }
        Ok(())
    }
}

impl Supervisor for BufferSupervisor {
    fn strategy(&self) -> RestartStrategy {
        RestartStrategy::OneForOne
    }

    fn children(&self) -> &[ChildSpec] {
        &self.children
    }

    fn handle_child_failure(&mut self, child_id: ActorId, reason: FailureReason) {
        let count = self.restart_counts.entry(child_id).or_insert(0);
        *count += 1;

        if *count > 5 {
            tracing::error!(?child_id, "Too many restarts, escalating");
            // Escalate to parent
        } else {
            tracing::warn!(?child_id, ?reason, "Restarting actor");
            // Restart actor
        }
    }
}
```

### Editor Supervision Tree

```rust
fn build_editor_supervisor() -> SupervisorBuilder {
    SupervisorBuilder::new()
        .child(ChildSpec::new("ui")
            .actor_type(ActorType::Supervisor)
            .restart_policy(RestartPolicy::Permanent))
        .child(ChildSpec::new("buffers")
            .actor_type(ActorType::Supervisor)
            .restart_policy(RestartPolicy::Permanent))
        .child(ChildSpec::new("lsp")
            .actor_type(ActorType::Supervisor)
            .restart_policy(RestartPolicy::Transient))
        .child(ChildSpec::new("terminal")
            .actor_type(ActorType::Worker)
            .restart_policy(RestartPolicy::Permanent))
}
```

## Best Practices

1. **Start simple** — One-for-one is usually sufficient
2. **Set limits** — Always configure max_restarts
3. **Use backoff** — Prevent restart storms
4. **Log failures** — Make failures observable
5. **Design for failure** — What happens if actor panics?
6. **Test supervision** — Inject failures deliberately
