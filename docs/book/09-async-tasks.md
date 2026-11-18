# Chapter 9: Async Tasks

Runact has a native async task executor for standard Rust `Future`s. This
chapter covers how to spawn, await, and cancel async tasks.

---

## 9.1 spawn_task

`Runtime::spawn_task` runs any `Future<Output = T> + Send + 'static` on
Runact's native executor:

```rust
use runact::Runtime;

let mut runtime = Runtime::new().unwrap();

let handle = runtime.spawn_task(async {
    // This runs on Runact's executor, not on an actor worker
    let result = some_async_operation().await;
    result
}).unwrap();

// Later, retrieve the result:
let result: String = handle.recv().unwrap();
```

### Key Guarantees

1. **Pending futures consume no worker time.** When a future returns
   `Poll::Pending`, the executor parks it and polls something else.
   A Runact-specific Waker re-queues the task when the future becomes
   ready (e.g., when a timer fires or I/O completes).

2. **Panics are caught.** A panic inside a future is caught and surfaced
   to the `TaskHandle` as `TaskError::Panic` — it does NOT kill a worker
   thread.

3. **Detachable.** Dropping the `TaskHandle` detaches the task: it
   continues running to completion, but you can no longer retrieve the
   result.

## 9.2 TaskHandle

`spawn_task` returns a `TaskHandle<T>`:

```rust
pub struct TaskHandle<T> {
    pub fn id(&self) -> TaskId;
    pub fn recv(&self) -> Result<T, TaskError> where T: Clone + 'static;
    pub fn try_recv(&self) -> Option<Result<T, TaskError>> where T: Clone + 'static;
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, TaskError> where T: Clone + 'static;
}
```

### recv — Block Until Done

```rust
let result = handle.recv().unwrap();  // blocks until task completes
```

### try_recv — Check Without Blocking

```rust
match handle.try_recv() {
    Some(Ok(result)) => { /* done */ }
    Some(Err(e)) => { /* error or panic */ }
    None => { /* still running */ }
}
```

### recv_timeout — Block With Timeout

```rust
match handle.recv_timeout(Duration::from_secs(5)) {
    Ok(result) => { /* got result */ }
    Err(TaskError::Timeout) => { /* too slow */ }
    Err(TaskError::Panic(msg)) => { /* task panicked */ }
    Err(TaskError::ExecutorShutdown) => { /* runtime shutting down */ }
}
```

### Output Must Be Clone

The `TaskHandle` caches the result so multiple `recv()` calls return it.
This requires `T: Clone`. If your output is not `Clone`, you must extract
it on the first `recv()` call and not call it again.

## 9.3 Async Sleep and Timeout

Runact provides async `sleep` and `timeout` as free functions:

```rust
use runact::Runtime;
use std::time::Duration;

let handle = runtime.spawn_task(async {
    Runtime::sleep(Duration::from_secs(1)).await;
    let result = Runtime::timeout(
        Duration::from_secs(5),
        async_http_request()
    ).await;
    result
}).unwrap();
```

- `Runtime::sleep(d)` — yields the task for `d` duration.
- `Runtime::timeout(d, future)` — runs `future` with a deadline of `d`.
  Returns `Ok(output)` if it completes in time, or `Err(TaskError::Timeout)`.

## 9.4 Bridging Actors and Async Tasks

> **Implementation status (v1.3.3):** `ActorContext` has no `spawn` method —
> actors cannot spawn async tasks directly. Spawn the task from outside the
> runtime (or from a thread holding a `RuntimeSender` for messaging) and send
> the result back as a normal message.

The supported pattern is: spawn from the owning thread, then deliver the
outcome to the actor with `Runtime::send`.

```rust
use runact::{Actor, ActorContext, ActorError, ActorId, Runtime, RuntimeSender};
use std::time::Duration;

// A message the actor understands for async completions.
enum HttpWork {
    Done(String),
    Failed(String),
}

struct HttpWorker;
impl Actor for HttpWorker {
    type Message = HttpWork;
    fn handle(&mut self, msg: HttpWork, _ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        match msg {
            HttpWork::Done(body) => println!("fetched {} bytes", body.len()),
            HttpWork::Failed(e) => eprintln!("fetch failed: {e}"),
        }
        Ok(())
    }
}

// Outside the actor: spawn the task and post the result back.
fn fetch_into(runtime: &Runtime, sender: RuntimeSender, id: ActorId, url: String) {
    runtime
        .spawn_task(async move {
            let result = fetch_url(&url).await;
            match result {
                Ok(body) => sender.send(id, HttpWork::Done(body)).ok(),
                Err(e) => sender.send(id, HttpWork::Failed(e.to_string())).ok(),
            };
        })
        .unwrap();
}
```

Timers can still be used to poll a task that was started elsewhere, but the
common pattern is the completion message shown above.

## 9.5 Task Groups and Cancellation

`TaskGroup` provides **structured concurrency** — a group of related
tasks shares one cancellation token:

```rust
use runact::{CancellationToken, Runtime, TaskGroup};

let runtime = Runtime::new().unwrap();
let group = TaskGroup::new();

// parent → child token propagation
let parent_token = CancellationToken::new();
let child_token = parent_token.child_token();

// Spawning through the group wraps the future with the group's token.
let handle = group.spawn(&runtime, async move {
    // Task checks child_token.is_cancelled() periodically
    // When parent_token.cancel() is called, child_token becomes cancelled
});

parent_token.cancel();
group.cancel(); // cancels every task spawned through the group
```

Cancellation is **cooperative** — tasks must check their token
`is_cancelled()` and exit gracefully. Runact does NOT forcibly kill
threads or futures.

> **Note (v1.3.3):** dropping a `TaskGroup` does not cancel its children.
> Call `group.cancel()` (or cancel the parent token) explicitly.
