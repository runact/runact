# Compute

## Overview

The compute scheduler offloads CPU-intensive work to a thread pool, keeping the actor scheduler responsive. Workers catch panics to isolate failures.

## Submitting Work from an Actor

```rust
use runact::{Actor, ActorContext, ActorError, ComputeHandle};
use std::time::Duration;

struct DataProcessor {
    pending: Option<ComputeHandle<Vec<u8>>>,
}

enum DataMsg {
    Process(Vec<u8>),
    CheckResult,
    GetResult,
}

impl Actor for DataProcessor {
    type Message = DataMsg;

    fn handle(&mut self, msg: DataMsg, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            DataMsg::Process(data) => {
                let handle = ctx.spawn_compute(move || {
                    // CPU-intensive work here — runs on compute pool, not actor worker
                    data.iter().map(|b| b.wrapping_add(1)).collect()
                }).expect("Failed to submit compute task");
                self.pending = Some(handle);
            }
            DataMsg::CheckResult => {
                if let Some(ref handle) = self.pending {
                    if let Some(result) = handle.try_recv() {
                        match result {
                            Ok(value) => println!("Processed: {} bytes", value.len()),
                            Err(e) => eprintln!("Compute failed: {:?}", e),
                            None => {} // Still running
                        }
                    }
                }
            }
            DataMsg::GetResult => {
                if let Some(ref handle) = self.pending {
                    match handle.recv_timeout(Duration::from_secs(1)) {
                        Ok(value) => {
                            ctx.reply(value).ok();
                            self.pending = None;
                        }
                        Err(e) => {
                            ctx.reply(e).ok();
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
```

## Submitting Work Externally

```rust
let handle = runtime.compute().spawn(|| {
    expensive_computation()
}).unwrap();

let result = handle.recv().unwrap();
```

## ComputeHandle Methods

| Method | Behavior |
|--------|----------|
| `try_recv()` | Non-blocking poll — `Some(Ok(T))`, `Some(Err)`, or `None` if still running |
| `recv()` | Block until result arrives |
| `recv_timeout(dur)` | Block up to timeout |
| `cancel()` | Request cancellation (checked before/after execution) |
| `is_cancelled()` | Check if cancellation was requested |

## Cancellation

```rust
let handle = ctx.spawn_compute(|| {
    long_running_task()
}).unwrap();

// Later, if we no longer need the result:
handle.cancel();
```

The worker checks the cancellation flag before and after execution. If set before: the task is skipped (`ComputeResult::Cancelled`). If set after: the result is discarded.

## Panic Isolation

Panics inside compute tasks are caught and surfaced as `ComputeError::WorkerPanic(msg)`. The worker thread survives and continues processing tasks.

```rust
let handle = runtime.compute().spawn(|| -> String {
    panic!("Compute task panicked!");
});

match handle.recv() {
    Err(runact::ComputeError::WorkerPanic(msg)) => {
        // Panic was caught, msg contains the panic message
    }
    _ => { /* ... */ }
}
```

## Configuration

```rust
use runact::{Runtime, RuntimeConfig, ComputeConfig};

let config = RuntimeConfig {
    compute: ComputeConfig {
        max_workers: 4,       // Defaults to available parallelism
        queue_capacity: 1024,
        task_timeout: None,   // Reserved for future use
    },
    ..Default::default()
};
```

## When to Use Compute

- **Use compute** for CPU-intensive work: parsing, encryption, compression, data transformation, mathematical computation
- **Don't use compute** for I/O-bound work (use timers instead) or for work that needs actor state — do that directly in the handler
- **Don't block** inside actors — submit to compute and poll the handle later
- **Always check cancellation** — the worker checks the flag before and after execution
