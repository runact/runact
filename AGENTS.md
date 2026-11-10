# Runact — Project Guide for Coding Agents

This file refines the global workflow (see `~/.config/opencode/AGENTS.md`) for this repository. Both apply; where they differ on mechanics, this file wins.

## Project Identity

Runact is a **Rust-native actor runtime** (v1.3.2, edition 2024, MSRV 1.85) — BEAM-style lightweight processes, messaging, scheduling, supervision, with Rust's ownership model. Workspace with `runact` (core) and `runact-web` (HTTP/WebSocket layer) members.

Runact's runtime extensions are a **native async task system** (standard Rust `Future` execution on its own executor), **TCP networking** for remote AI agents, and **hot code loading** (feature-gated). See:

- `docs/async-runtime.md` — Async runtime boundary document
- `docs/adr/0003-native-async-runtime.md` — ADR for native Future executor
- `docs/runtime-networking-plan.md` — Full plan for TCP, process runtime, cancellation, task groups, and remote AI agent support

> Runact owns concurrency and task lifecycle. Specialized libraries provide HTTP, WebSocket, TLS, DNS, and application-level protocols. **No Tokio in the core — ever.**

## Repository Layout

```text
src/
├── lib.rs          # Public module declarations + crate-root re-exports
├── actor/          # Actor trait, ActorId, ActorContext, ActorError (public)
├── compute/        # ComputeScheduler, ComputeHandle (public via root re-export)
├── hotswap/        # Hot code loading: CodeManager, ActorFactory, DynamicSupervisor,
│                   # SpawnListener, MigrationRegistry (public, feature `hot_code_loading`)
├── mailbox/        # Bounded mailbox + backpressure (private)
├── net/            # TCP reactor (epoll), TcpListener, TcpStream (public)
├── process/        # Process spawning with stdin/stdout/stderr, timeout, cancellation (public)
├── resource/       # Capability, ResourceHandle, ResourceRegistry (public)
├── runtime.rs      # Runtime, RuntimeConfig, RuntimeStats, RequestHandle (private module)
├── scheduler/      # BEAM-style scheduler, work stealing, reductions (private)
├── supervision/    # Supervisor, ChildSpec, RestartStrategy (public via root re-export)
├── task/           # Async task execution: TaskCell, Executor, TaskHandle, TaskError (public)
├── timer/          # Timer, TimerService (public)
└── error.rs        # RuntimeError

tests/              # Integration tests — one file per feature area (actor_task_integration,
                    # async_executor, async_timers, basic, cancellation, compute, observability,
                    # process_runtime, reactor, resource, scheduler, stress, tcp_api, timer).
                    # These are the ACCEPTANCE layer: behavioral, hitting real entry points.
                    # Hot code loading unit tests live inline in src/hotswap/tests.rs and run
                    # only with `--features hot_code_loading`.
benches/runact_bench.rs   # Criterion benchmarks (harness = false)
docs/               # architecture.md, async-runtime.md, runtime-networking-plan.md,
                    # hot-code-loading.md, adr/, guides/, book/, etc.
runact-web/         # HTTP/WebSocket layer (separate crate, workspace member)
```

## Mandatory Workflow

1. **Read the spec / acceptance criterion first.** If none exists, ask — never invent requirements.
2. **Design the failing test before the code.**
   - *Behavioral/acceptance tests* → `tests/<area>.rs` (one file per feature area, e.g. `tests/async_executor.rs` for a new async slice). Hit real entry points, assert observable behavior.
   - *Unit tests for core logic* → inline `#[cfg(test)]` modules in the `src/` file under test.
   - Run them → they MUST FAIL (red). If they pass, the test is wrong or the feature exists.
   - NEVER modify existing tests written for previously accepted behavior — they are the contract.
3. **Implement the minimal code** that makes them pass. Follow existing patterns — read sibling files first (`runtime.rs`, `compute/`, `timer/` are the reference style).
4. **Run the full gate**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, `cargo test --doc`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`. CI enforces all of these on `main`.
5. **Report**: files changed, test counts, red→green proof.

## Hard Rules

- **"Done" = full gate green + no scope creep.** Nothing else counts.
- **No test, no feature.** Every requirement maps to a test that fails first.
- **Fix the implementation, NEVER the test.** A failing test means the code is wrong.
- **No new dependencies** (core deps are exactly: `crossbeam-channel`, `libc`, `serde`, `tracing`, `thiserror`, plus the optional `libloading` behind `hot_code_loading`). Any addition needs an explicit architectural justification in the docs first.
- **No Tokio** in core dependencies. Specialized I/O libraries belong to applications, not the crate root.
- **No type-error suppression** (`as any`/`unsafe`/`.unwrap()` in library code) — this is a `-D warnings` clippy/rustdoc gate anyway.
- **Do not commit** unless explicitly asked.
- **Update docs when behavior changes**: `docs/architecture.md` holds invariants; plan/design docs (`docs/async-runtime.md`, `docs/runtime-networking-plan.md`, ADRs) are updated by decision, not afterthought. CHANGELOG follows Keep a Changelog.
- **Async runtime invariants** (from `docs/async-runtime.md` §22) are mandatory once the executor lands: pending tasks consume no worker time; CPU-heavy work never runs on async/actor workers; actors never block a worker waiting for I/O; task failure never terminates the runtime; cancellation is cooperative; shutdown deterministically terminates owned tasks; actors and async tasks remain fairly scheduled.
- **Networking invariants** (from `docs/runtime-networking-plan.md` §30): Runact is NOT an HTTP/WebSocket/TLS/AI framework; TCP is the lowest-level network primitive; actors and async tasks remain distinct; structured concurrency prevents orphaned tasks; process cancellation prevents orphaned child processes; Tokio is optional, not fundamental.

## Conventions

- Follow the architecture: actors are synchronous and cooperative (`fn handle(&mut self, msg, ctx)`); async is for I/O-waiting; compute pool is for CPU. An actor must never occupy a scheduler worker while waiting.
- Error types via `thiserror`; every public type `#[must_use]`; structured logging via `tracing` (never `println!`/`eprintln!` in library code).
- Ownership, not `Arc<Mutex<…registry>>` — see `docs/architecture.md` §32 (async plan) / ownership model.
- Keep the public API small. `Runtime::new/spawn/send/request/shutdown` are the surface; scheduler internals stay private.
- Benchmarks are criterion in `benches/runact_bench.rs`; run them when perf-affecting changes land. Async runtime perf targets are engineering goals, not guarantees (see `docs/async-runtime.md`).
- MSRV 1.85 — do not use language features newer than 1.85 even if the toolchain is newer.

## Integration Tests (Acceptance Layer)

Integration tests live in `runact-web/tests/` for the web layer and `tests/` for core.
Each file tests one feature area. Run all with `cargo test --all-targets`.

### Test Counts

| Crate | Tests | Suites |
|-------|-------|--------|
| runact | 110 | lib unit (7) + actor_task_integration, async_executor, async_timers, basic, cancellation, compute, observability, process_runtime, reactor, resource, scheduler, stress, tcp_api, timer (14 suites) |
| runact (hot swap) | +9 | `src/hotswap/tests.rs`, run with `--features hot_code_loading` (119 total) |
| runact-web | 89 | WebSocket: ws_server, ws_chat, ws_runact_bridge, ws_fragmentation, ws_agent_server, ws_agent_api, async_websocket, websocket |
|        |       | HTTP: http_request, http_response, headers, router, middleware_extractors |
| Total | 199 across 27 suites | 0 failures (208 with `hot_code_loading`)

## Definition of Done
- [ ] Minimal implementation, matching sibling-file patterns
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo test --all-targets` green (no pre-existing failures introduced)
- [ ] `cargo test --doc` green, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` clean
- [ ] Docs/CHANGELOG updated if public API or documented behavior changed
- [ ] Report: files changed, test counts, red→green proof