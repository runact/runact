# ADR-0001: BEAM-Style Scheduler from v1

## Status

Accepted

## Context

Runact is a minimal BEAM-inspired actor runtime in Rust. The original roadmap used Tokio for scheduling initially, with a custom scheduler planned for v2. The user wants BEAM-style scheduling from v1.

## Decision

Use a custom BEAM-style scheduler with work stealing from v1. No Tokio scheduler dependency.

## Rationale

### Why BEAM-style from v1?

1. **Correctness** — BEAM scheduling is well-understood and proven for actor systems
2. **Performance** — Work stealing provides better load balancing than static assignment
3. **Fairness** — Reduction counting prevents starvation
4. **Scalability** — Per-worker run queues reduce contention
5. **Predictability** — Custom scheduler gives full control over scheduling decisions

### Why not Tokio?

1. **Leaky abstraction** — Tokio types would leak through public APIs
2. **Different model** — Tokio is task-based, not process-based
3. **Less control** — Cannot implement reduction counting with Tokio
4. **Overhead** — Tokio adds unnecessary overhead for simple message passing

## Consequences

### Positive

- Full control over scheduling behavior
- Better performance characteristics for actor systems
- No Tokio types in public APIs
- Reduction counting prevents starvation
- Work stealing provides natural load balancing

### Negative

- More code to write and maintain
- Need to handle I/O integration ourselves
- Need to handle timers ourselves
- Need to handle thread management ourselves

## Implementation

### Core Components

1. **RunQueue** — Lock-free MPSC queue per worker
2. **Worker** — Per-core scheduler with own run queue
3. **Stealer** — Work stealing between workers
4. **ReductionCounter** — Prevents starvation
5. **Scheduler** — Top-level coordinator

### Algorithm

```
1. Worker checks own run queue
2. If empty, try to steal from random worker
3. If still empty, park (block until new work)
4. When running process:
   a. Process one message
   b. Increment reduction counter
   c. If counter >= MAX_REDUCTIONS, preempt
   d. Otherwise, continue with next message
```

### Configuration

- `num_workers`: Number of worker threads (default: available parallelism)
- `max_reductions`: Reductions before preemption (default: 4000)
- `steal_batch_size`: Number of tasks to steal at once (default: half of victim's queue)

## References

- BEAM scheduler documentation
- Rust crossbeam-deque for work stealing
- Tokio work stealing (for comparison)
