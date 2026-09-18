# Getting Started with Runact

> Runact is a Rust-native actor runtime and application platform inspired by BEAM.
> It provides lightweight actors, async tasks, supervision, timers, process management,
> and TCP networking — all without Tokio in the core.

## Installation

Add Runact to your `Cargo.toml`:

```toml
[dependencies]
runact = "1.2"
```

Minimum supported Rust version: **1.85** (edition 2024).

---

## 1. Your First Actor

An actor is an isolated unit of state and behavior. It owns its mutable state and
communicates through typed messages — no shared memory.

```rust
use runact::{Actor, ActorContext, ActorError, Runtime};

struct Greeter;

impl Actor for Greeter {
    type Message = String;

    fn handle(&mut self, msg: String, _ctx: &mut ActorContext) -> Result<(), ActorError> {
        println!("Hello, {}!", msg);
        Ok(())
    }
}

fn main() {
    let mut runtime = Runtime::new().unwrap();
    let id = runtime.spawn(Greeter).unwrap();
    runtime.send(id, "world".to_string()).unwrap();
    runtime.shutdown().unwrap();
}
```

**Key points:**
- `Actor` trait requires `Send + 'static` — actors own all their data.
- `Message` must be `Send + 'static` — messages are transferred between threads.
- `handle()` receives one message at a time — no concurrent mutation.
- `spawn()` returns an `ActorId` — a stable, unique identifier.

---

## 2. Sending Messages

### Fire-and-Forget

Non-blocking. Returns `Err(MailboxFull)` if the actor's mailbox is at capacity (default: 1000).

```rust
runtime.send(id, "hello".to_string()).unwrap();
```

### Blocking Send (External Threads Only)

Blocks until the mailbox accepts the message. **Never call from inside an actor** — it would
block the scheduler worker.

```rust
runtime.send_blocking(id, "hello".to_string()).unwrap();
```

---

## 3. Request-Reply

Send a message and receive a response:

```rust
use runact::{Actor, ActorContext, ActorError, Runtime};
use std::time::Duration;

struct Echo;

impl Actor for Echo {
    type Message = String;

    fn handle(&mut self, msg: String, ctx: &mut ActorContext) -> Result<(), ActorError> {
        if ctx.is_request() {
            ctx.reply(format!("echo: {}", msg))?;
        }
        Ok(())
    }
}

fn main() {
    let mut runtime = Runtime::new().unwrap();
    let id = runtime.spawn(Echo).unwrap();

    // Send a request and get a handle
    let handle = runtime.request(id, "hello".to_string()).unwrap();

    // Wait for the reply (with timeout)
    let reply = handle.recv_timeout(Duration::from_secs(1)).unwrap();
    let text = reply.downcast::<String>().unwrap();
    println!("Got: {}", *text);

    runtime.shutdown().unwrap();
}
```

### RequestHandle Methods

| Method | Behavior |
|--------|----------|
| `recv()` | Block until reply arrives |
| `try_recv()` | Return immediately — `Ok` or `Err` |
| `recv_timeout(dur)` | Block up to `dur`, then return error |

---

## 4. Actor-to-Actor Messaging

Inside an actor, use `ctx.send_to()` to forward messages:

```rust
use runact::{Actor, ActorContext, ActorError, ActorId, Runtime};

struct Forwarder {
    target: ActorId,
}

impl Actor for Forwarder {
    type Message = String;

    fn handle(&mut self, msg: String, ctx: &mut ActorContext) -> Result<(), ActorError> {
        ctx.send_to(self.target, msg)?;
        Ok(())
    }
}
```

---

## 5. Supervision

Supervisors manage actor lifecycles. When a child crashes, the supervisor restarts it
based on its strategy.

```rust
use runact::{Runtime, RestartStrategy, ChildSpec, RestartPolicy};
use std::time::Duration;

fn main() {
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
    runtime.shutdown().unwrap();
}
```

### Restart Strategies

| Strategy | Behavior |
|----------|----------|
| `OneForOne` | Only the failed child is restarted |
| `OneForAll` | All children are restarted |
| `RestForOne` | Failed child + children started after it are restarted |

### Restart Policies

| Policy | Behavior |
|--------|----------|
| `Permanent` | Always restart on crash |
| `Transient` | Restart only on abnormal exit (not clean `Ok(())`) |
| `Temporary` | Never restart |

---

## 6. Async Tasks

Runact has a native executor for standard Rust `Future`s — no Tokio required.

```rust
use runact::Runtime;
use std::time::Duration;

async fn compute_value() -> i64 {
    // Async work here
    42
}

fn main() {
    let runtime = Runtime::new().unwrap();

    // Spawn an async task
    let handle = runtime.spawn_task(compute_value()).unwrap();

    // Get the result (blocking)
    let result = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    println!("Result: {}", *result.downcast::<i64>().unwrap());

    runtime.shutdown().unwrap();
}
```

### Sleep and Timeout

```rust
use runact::Runtime;
use std::time::Duration;

async fn delayed_work() -> String {
    // Sleep inside an async task
    Runtime::sleep(Duration::from_secs(1)).await;

    "done".to_string()
}

fn main() {
    let runtime = Runtime::new().unwrap();

    // Run with a timeout
    let result = runtime.timeout(Duration::from_millis(500), delayed_work());
    let handle = runtime.spawn_task(result).unwrap();

    match handle.recv_timeout(Duration::from_secs(2)) {
        Ok(reply) => {
            if let Ok(val) = reply.downcast::<Result<String, runact::TaskError>>() {
                println!("Result: {:?}", *val);
            }
        }
        Err(_) => println!("Timed out"),
    }

    runtime.shutdown().unwrap();
}
```

### TaskHandle Methods

| Method | Behavior |
|--------|----------|
| `recv()` | Block until task completes |
| `try_recv()` | Return immediately — `Ok` or `Err` |
| `recv_timeout(dur)` | Block up to `dur`, then return error |

---

## 7. Cancellation

Cooperative cancellation with parent→child propagation.

```rust
use runact::{CancellationToken, Runtime};
use std::time::Duration;

async fn long_task(token: CancellationToken) {
    for i in 0..100 {
        if token.is_cancelled() {
            println!("Cancelled at step {}", i);
            return;
        }
        Runtime::sleep(Duration::from_millis(100)).await;
    }
}

fn main() {
    let runtime = Runtime::new().unwrap();
    let token = CancellationToken::new();

    let child_token = token.child_token();
    let handle = runtime.spawn_task(long_task(child_token)).unwrap();

    // Cancel after 300ms
    std::thread::sleep(Duration::from_millis(300));
    token.cancel();

    // Task will observe cancellation on next poll
    let _ = handle.recv_timeout(Duration::from_secs(2));
    runtime.shutdown().unwrap();
}
```

### TaskGroup

Structured concurrency — all tasks in the group are cancelled when the scope exits.

```rust
use runact::{TaskGroup, Runtime};
use std::time::Duration;

async fn worker(id: u32) {
    loop {
        println!("Worker {} tick", id);
        Runtime::sleep(Duration::from_millis(200)).await;
    }
}

fn main() {
    let runtime = Runtime::new().unwrap();

    runtime.spawn_task(async {
        let mut group = TaskGroup::new();
        group.spawn(worker(1));
        group.spawn(worker(2));

        // All tasks run until we cancel the group
        Runtime::sleep(Duration::from_secs(1)).await;
        group.cancel_all();
    }).unwrap();

    std::thread::sleep(Duration::from_secs(2));
    runtime.shutdown().unwrap();
}
```

---

## 8. Compute Pool

Offload CPU-intensive work to a thread pool, keeping the actor scheduler responsive.

```rust
use runact::{Actor, ActorContext, ActorError, Runtime};
use std::time::Duration;

struct DataProcessor {
    pending: Option<runact::ComputeHandle<Vec<u8>>>,
}

enum DataMsg {
    Process(Vec<u8>),
    CheckResult,
}

impl Actor for DataProcessor {
    type Message = DataMsg;

    fn handle(&mut self, msg: DataMsg, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            DataMsg::Process(data) => {
                let handle = ctx.spawn_compute(move || {
                    // CPU-intensive work
                    data.iter().map(|b| b.wrapping_add(1)).collect()
                })?;
                self.pending = Some(handle);
            }
            DataMsg::CheckResult => {
                if let Some(ref handle) = self.pending {
                    if let Some(result) = handle.try_recv() {
                        println!("Processed {} bytes", result.len());
                    }
                }
            }
        }
        Ok(())
    }
}
```

---

## 9. Timers

### One-Shot Timer

```rust
use std::time::Duration;

// From outside an actor:
runtime.schedule_timer(Duration::from_secs(5), actor_id, "wake up".to_string());

// From inside an actor:
ctx.schedule_timer(Duration::from_millis(500), "ping".to_string())?;
```

### Periodic Timer

```rust
use std::time::Duration;

// From inside an actor:
ctx.schedule_interval(Duration::from_secs(1), Tick)?;
```

### Cancel a Timer

```rust
let timer_id = ctx.schedule_timer(Duration::from_secs(10), "delayed".to_string()).unwrap();
ctx.cancel_timer(timer_id);
```

---

## 10. Process Management

Spawn and manage OS processes with stdin/stdout/stderr.

```rust
use runact::process::{ProcessSpawnOptions, ProcessHandle};
use std::time::Duration;

fn main() {
    // Spawn a process
    let mut child = ProcessSpawnOptions::new("echo")
        .arg("hello from runact")
        .stdout_piped()
        .spawn()
        .unwrap();

    // Read output
    let line = child.read_stdout_line().unwrap();
    println!("Got: {:?}", line);

    // Wait for completion with timeout
    let output = child.wait_timeout(Duration::from_secs(5)).unwrap();
    println!("Exit code: {:?}", output.exit_code);
    println!("Stdout: {}", output.stdout);
}
```

### ProcessHandle Methods

| Method | Behavior |
|--------|----------|
| `write_stdin(data)` | Write bytes to stdin |
| `close_stdin()` | Close stdin (signals EOF) |
| `read_stdout_line()` | Read one line from stdout (blocking) |
| `kill()` | Kill the process |
| `wait()` | Wait for exit, collect output |
| `wait_timeout(dur)` | Wait with timeout, kill on expiry |

Processes are automatically killed and waited on when `ProcessHandle` is dropped.

---

## 11. TCP Networking

Non-blocking TCP with epoll-based reactor (Linux).

### TCP Server

```rust
use runact::net::tcp_api::TcpListener;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());

    // Accept a connection
    let mut stream = listener.accept().unwrap();

    // Write response
    stream.write_all(b"HTTP/1.1 200 OK\r\n\r\nHello from Runact!").unwrap();
    stream.flush().unwrap();

    // Read client data
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap();
    println!("Got {} bytes", n);
}
```

### TCP Client

```rust
use runact::net::tcp_api::TcpStream;

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:8080").unwrap();

    stream.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    stream.flush().unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    println!("Response: {}", String::from_utf8_lossy(&response));
}
```

### TcpStream Methods

| Method | Behavior |
|--------|----------|
| `connect(addr)` | Connect to a remote address |
| `read(buf)` | Read into buffer (non-blocking) |
| `read_exact(buf)` | Read exactly `buf.len()` bytes |
| `read_to_end(buf)` | Read until EOF |
| `write_all(buf)` | Write all bytes |
| `flush()` | Flush write buffer |
| `shutdown(how)` | Shutdown read/write/both |

---

## 12. Resources & Capabilities

Type-safe, capability-based resource management.

```rust
use runact::{ResourceHandle, Capability, ResourceRegistry};
use std::any::Any;

#[derive(Debug)]
struct DatabaseConnection {
    url: String,
}

impl ResourceHandle for DatabaseConnection {
    fn resource_type(&self) -> &str { "DatabaseConnection" }
    fn as_any(&self) -> &dyn Any { self }
}

fn main() {
    let registry = ResourceRegistry::new();

    // Register a capability
    let cap = Capability::new(
        actor_id,
        DatabaseConnection { url: "postgres://localhost/mydb".to_string() }
    );
    registry.register(cap);

    // Retrieve by type
    let retrieved: Option<Capability<DatabaseConnection>> = registry.get();
    assert!(retrieved.is_some());
}
```

---

## 13. Runtime Configuration

```rust
use runact::{Runtime, RuntimeConfig, ComputeConfig};
use std::time::Duration;

let config = RuntimeConfig {
    compute: ComputeConfig {
        max_workers: 4,
        queue_capacity: 1024,
        task_timeout: None,
    },
    mailbox_capacity: 2000,
    shutdown_timeout: Duration::from_secs(10),
};

let mut runtime = Runtime::with_config(config).unwrap();
```

---

## 14. Observability

All actor lifecycle events are logged via `tracing`:

| Level | Events |
|-------|--------|
| `info` | Actor spawned, actor stopped |
| `debug` | Message sent, request sent |
| `trace` | Per-message handling |
| `warn` | Actor restarted |
| `error` | Max restarts exceeded |

```rust
let stats = runtime.stats();
println!("Actors: {}, Requests: {}", stats.actor_count, stats.request_count);
```

Enable logging in your application:

```toml
[dependencies]
tracing-subscriber = "0.3"
```

```rust
tracing_subscriber::fmt::init();
```

---

## 15. Shutdown

```rust
runtime.shutdown().unwrap();
```

Shutdown is also called automatically when `Runtime` is dropped. The shutdown sequence:

1. Stop accepting new messages
2. Drain remaining mailboxes
3. Wait for all actors to finish current message
4. Stop compute workers
5. Resolve pending task handles to `TaskError::ExecutorShutdown`

---

## Complete Example: Web Worker Pool

A realistic example combining actors, supervision, and compute:

```rust
use runact::{Actor, ActorContext, ActorError, Runtime, RestartStrategy, ChildSpec, RestartPolicy};
use std::time::Duration;

struct Worker {
    id: u32,
}

enum WorkerMsg {
    ProcessJob(String),
    HealthCheck,
}

impl Actor for Worker {
    type Message = WorkerMsg;

    fn handle(&mut self, msg: WorkerMsg, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            WorkerMsg::ProcessJob(job) => {
                println!("Worker {} processing: {}", self.id, job);
                // Offload CPU work to compute pool
                let id = self.id;
                ctx.spawn_compute(move || {
                    // Simulate work
                    std::thread::sleep(Duration::from_millis(100));
                    format!("Worker {} done with: {}", id, job)
                })?;
            }
            WorkerMsg::HealthCheck => {
                println!("Worker {} healthy", self.id);
            }
        }
        Ok(())
    }
}

fn main() {
    let mut runtime = Runtime::new().unwrap();

    // Supervised worker pool
    let strategy = RestartStrategy::OneForOne {
        max_restarts: 5,
        within: Duration::from_secs(60),
        base_backoff: Duration::from_millis(100),
    };

    let children: Vec<ChildSpec> = (0..4)
        .map(|i| {
            ChildSpec::new(format!("worker-{}", i))
                .restart_policy(RestartPolicy::Permanent)
        })
        .collect();

    let supervisor = runtime.spawn_supervisor(strategy, children).unwrap();

    // Send jobs
    for i in 0..10 {
        runtime.send(supervisor, WorkerMsg::ProcessJob(format!("job-{}", i))).unwrap();
    }

    // Periodic health checks
    runtime.schedule_interval(
        Duration::from_secs(5),
        supervisor,
        WorkerMsg::HealthCheck,
    );

    // Run for a while
    std::thread::sleep(Duration::from_secs(10));
    runtime.shutdown().unwrap();
}
```

---

## Next Steps

- [Architecture](../architecture.md) — Full architectural document
- [Actor Communication](../actor-communication.md) — 24 principles for message flow
- [Async Runtime](../async-runtime.md) — Async executor design and invariants
- [Supervision](supervision.md) — Supervision trees in depth
- [Compute](compute.md) — CPU-intensive work offloading
- [Timers](timers.md) — One-shot and periodic timers
- [Resources](resources.md) — Capability-based resource management
- [Roadmap](../roadmap.md) — Development phases and what's coming next
