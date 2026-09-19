# Chapter 11: Compute Pool

The compute pool is a separate thread pool for CPU-intensive work. It
ensures that heavy computations never block the scheduler workers that
run actors and async tasks.

---

## 11.1 Why a Separate Pool?

Runact's scheduler uses a fixed number of worker threads (one per CPU
core by default). These workers run both actor handlers and async task
polls. If a single `handle` call takes 5 seconds to compute a SHA-256
hash over 10MB of data, that worker is stuck — and up to N-1 other
workers may also be idle while waiting for that actor's messages.

The **compute pool** solves this: CPU-bound work is submitted to a
separate pool of worker threads, and the actor is free to continue
processing other messages.

```
Actor worker ──spawn_compute()──► Compute pool ──(busy)──► Actor worker
     (free)                        (dedicated CPU threads)    (receives result)
```

## 11.2 ctx.spawn_compute

Inside an actor, use `ctx.spawn_compute`:

```rust
use runact::{Actor, ActorContext, ActorError};

struct Hasher;

impl Actor for Hasher {
    type Message = HashRequest;

    fn handle(&mut self, msg: HashRequest, ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        // This closure runs on the compute pool, not on the actor worker
        let handle = ctx.spawn_compute(move || {
            sha256_heavy_computation(&msg.data)
        })?;

        // Store the handle to poll later
        self.pending.insert(msg.request_id, handle);
        Ok(())
    }
}
```

### Requirements

- The closure must be `FnOnce() -> T + Send + 'static`.
- The return type `T` must be `Send + 'static`.
- No access to `&self` — the closure is moved to another thread.

## 11.3 ComputeHandle<T>

`spawn_compute` returns a `ComputeHandle<T>`:

```rust
pub struct ComputeHandle<T> {
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
    pub fn try_recv(&self) -> Option<Result<T, ComputeError>> where T: 'static;
    pub fn recv(&self) -> Result<T, ComputeError> where T: 'static;
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, ComputeError> where T: 'static;
}
```

### Polling for Results

```rust
// Check if done (non-blocking)
match handle.try_recv() {
    Some(Ok(result)) => { /* use result */ }
    Some(Err(e)) => { /* handle error */ }
    None => { /* still running — try again later */ }
}

// Wait for result (blocks — but only on compute pool thread if called from actor)
match handle.recv_timeout(Duration::from_secs(10)) {
    Ok(result) => { /* use result */ }
    Err(ComputeError::WorkerPanic(msg)) => { /* compute panicked */ }
    Err(ComputeError::SchedulerShutdown) => { /* runtime shutting down */ }
}
```

## 11.4 Cancellation

`ComputeHandle::cancel` sets a flag that the compute worker checks
before executing the task:

```rust
let handle = ctx.spawn_compute(|| expensive_thing())?;

// If the actor receives a Cancel message before the task starts:
handle.cancel();

// Later, check:
if handle.is_cancelled() {
    // Task was cancelled, won't produce a result
}
```

Cancellation is **best-effort**: if the task is already running, `cancel`
won't stop it — it only prevents tasks that haven't started yet. For
long-running compute jobs, the closure should check an external
cancellation flag periodically.

## 11.5 ComputeConfig

The compute pool is configured via `RuntimeConfig`:

```rust
use runact::{Runtime, RuntimeConfig, ComputeConfig};

let config = RuntimeConfig {
    compute: ComputeConfig {
        max_workers: 4,       // defaults to available_parallelism
        queue_capacity: 1024, // bounded — backpressure when full
        task_timeout: None,
    },
    ..RuntimeConfig::default()
};
let mut runtime = Runtime::with_config(config).unwrap();
```

### Queue Capacity and Backpressure

When the compute queue is full and you call `spawn_compute`, it returns
`ComputeError::QueueFull`. The actor must handle this:

```rust
match ctx.spawn_compute(|| heavy_work()) {
    Ok(handle) => { self.pending.insert(id, handle); }
    Err(ComputeError::QueueFull) => {
        // Retry later or reject the request
    }
    Err(e) => {
        return Err(ActorError::Handler(e.to_string()));
    }
}
```

## 11.6 Use Cases for Compute Pool

| Task | Why compute pool? |
|------|---------------------|
| JSON encoding/decoding (large payloads) | CPU-bound, no I/O |
| Image/video processing | Heavy computation |
| Cryptographic hashing | CPU-intensive |
| Compression/decompression | CPU-bound |
| Parsing (JSON, CSV, etc.) | CPU-intensive for large inputs |
| Mathematical computation | Pure CPU |
| Code indexing/search | CPU-bound batch work |

## 11.7 ComputePool vs AsyncTask

| Feature | Compute pool (`spawn_compute`) | Async task (`spawn_task`) |
|---------|-------------------------------|--------------------------|
| Runs on | Compute worker threads | Async executor threads |
| Closure type | `FnOnce() -> T` | `Future<Output = T>` |
| Blocking OK? | Yes (it's a dedicated thread) | No (blocks worker) |
| Use case | CPU-bound work | I/O-bound work |
| Result handle | `ComputeHandle<T>` | `TaskHandle<T>` |
| Cancellation | `cancel()` (best-effort) | `CancellationToken` |

**Rule of thumb**: If it doesn't `await`, it goes to the compute pool.
If it `awaits` on I/O, it goes to the async executor.
