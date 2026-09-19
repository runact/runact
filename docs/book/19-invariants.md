# Chapter 19: Invariants

This chapter aggregates the runtime and networking invariants that all
Runact code must uphold. These are enforced by tests, clippy lints, and
the architecture itself.

---

## 19.1 Async Runtime Invariants

From `docs/async-runtime.md` §22:

1. **Pending tasks consume no worker time.**
   A future that returns `Poll::Pending` is parked until its Waker fires.
   The worker moves on to another task.

2. **CPU-heavy work never runs on async/actor workers.**
   CPU-bound code must be submitted to `ctx.spawn_compute`, which runs on
   the dedicated compute pool.

3. **Actors never block a worker waiting for I/O.**
   An actor's `handle` must return quickly. Long-running operations
   (HTTP requests, file I/O) must be offloaded to async tasks or the
   compute pool.

4. **Task failure never terminates the runtime.**
   A panic in an async task is caught by the executor and surfaced as
   `TaskError::Panic` on the `TaskHandle`.

5. **Cancellation is cooperative.**
   A `CancellationToken` signals cancellation, but the task must check
   `is_cancelled()` and exit gracefully. The runtime does not force-kill
   threads or futures.

6. **Shutdown deterministically terminates owned tasks.**
   When `Runtime::shutdown` is called, all spawned tasks, actors, and
   compute jobs are cancelled or joined with a timeout. The caller does
   not return until this is complete.

7. **Actors and async tasks remain fairly scheduled.**
   Reduction counting and task budgets ensure neither actors nor async
   tasks can starve each other.

## 19.2 Networking Invariants

From `docs/runtime-networking-plan.md` §30:

1. **Runact is NOT an HTTP/WebSocket/TLS/AI framework.**
   The core only provides TCP, process management, and I/O readiness.
   HTTP, WebSocket, TLS, and protocol handling live in `runact-web` or
   application code.

2. **TCP is the lowest-level network primitive.**
   All networking in runact core is socket-based TCP. Higher-level
   protocols are layered on top.

3. **Actors and async tasks remain distinct.**
   Actors communicate via messages; async tasks return values. They do
   not merge into a single abstraction.

4. **Structured concurrency prevents orphaned tasks.**
   All spawned async tasks are reachable via `TaskHandle` for the
   lifetime of the runtime. `TaskGroup` enables scoped cancellation.

5. **Process cancellation prevents orphaned child processes.**
   When a process actor is cancelled, its child processes are also
   terminated.

6. **Tokio is optional, not fundamental.**
   Runact's executor is native. Applications can use Tokio for specific
   I/O libraries, but the core does not depend on it.

7. **Runact must remain useful independently of PaperOS.**
   The runtime must be usable for general-purpose applications, not just
   the PaperOS editor.

## 19.3 Hard Rules (from AGENTS.md)

1. **No new dependencies** (core deps: `crossbeam-channel`, `serde`,
   `tracing`, `thiserror`).

2. **No Tokio in core.**

3. **No type-error suppression** (`as any` / `unsafe` / `.unwrap()`
   in library code).

4. **Every public type is `#[must_use]`.**

5. **No `println!`/`eprintln!` in library code** — use `tracing`.

6. **Error types use `thiserror`.**

7. **MSRV 1.85** — no language features newer than 1.85.

## 19.4 Verification

All invariants are verified by:
- Integration tests in `tests/` and `runact-web/tests/`
- `cargo clippy --all-targets -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`
- The `verify-rust.sh` script (if present)

To run the full gate:
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --doc
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```
