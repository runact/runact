//! Acceptance tests for the Runact async executor (Phases 1 + 2 of
//! `docs/async-runtime.md`): standard Rust `Future` execution on a native
//! Runact executor with wake deduplication.
//!
//! Covered behaviors (see doc §21):
//! - Future completion
//! - Future Pending → Wake → Resume
//! - Multiple tasks
//! - Wake deduplication
//! - Async task panic isolation (§16)
//! - 10,000+ tasks
//! - Structured runtime shutdown (§12, §15)

use runact::{Runtime, TaskError};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

/// A future that stays `Pending` until externally signalled, storing its
/// waker so the test can wake it deterministically.
struct ManualFuture {
    waker_slot: Arc<Mutex<Option<Waker>>>,
    signal: Arc<AtomicBool>,
    polls: Arc<AtomicUsize>,
    value: u64,
}

impl Future for ManualFuture {
    type Output = u64;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u64> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        if self.signal.load(Ordering::SeqCst) {
            Poll::Ready(self.value)
        } else {
            *self.waker_slot.lock().expect("lock waker slot") = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// A future that self-wakes on every `Pending` poll, advancing a counter.
/// With wake deduplication it must be polled exactly `target` times —
/// never more, no matter how many wakes race.
struct CountingFuture {
    polls: Arc<AtomicUsize>,
    target: usize,
}

impl Future for CountingFuture {
    type Output = usize;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<usize> {
        let n = self.polls.fetch_add(1, Ordering::SeqCst) + 1;
        if n >= self.target {
            Poll::Ready(n)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// Wait until `polled` reports the task has been polled at least `at_least`
/// times, or fail after a generous deadline.
fn wait_until_polled(polls: &AtomicUsize, at_least: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while polls.load(Ordering::SeqCst) < at_least && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(
        polls.load(Ordering::SeqCst) >= at_least,
        "task was never polled"
    );
}

#[test]
fn test_task_completes_with_result() {
    let runtime = Runtime::new().expect("create runtime");

    let handle = runtime.spawn_task(async { 42u64 }).expect("spawn task");

    assert_eq!(
        handle.recv_timeout(Duration::from_secs(2)).expect("recv"),
        42
    );
}

#[test]
fn test_pending_future_resumes_on_wake() {
    let runtime = Runtime::new().expect("create runtime");

    let waker_slot = Arc::new(Mutex::new(None::<Waker>));
    let signal = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicUsize::new(0));

    let handle = runtime
        .spawn_task(ManualFuture {
            waker_slot: waker_slot.clone(),
            signal: signal.clone(),
            polls: polls.clone(),
            value: 7,
        })
        .expect("spawn task");

    // First poll: task is pending and registered its waker.
    wait_until_polled(&polls, 1);
    assert!(
        handle.try_recv().is_none(),
        "task must still be pending before wake"
    );

    // External event: signal the value, then wake the task.
    signal.store(true, Ordering::SeqCst);
    let waker = waker_slot
        .lock()
        .expect("lock waker slot")
        .take()
        .expect("waker was not registered");
    waker.wake();

    assert_eq!(
        handle.recv_timeout(Duration::from_secs(2)).expect("recv"),
        7,
        "pending future must resume after wake"
    );
}

#[test]
fn test_self_waking_future_is_not_overpolled() {
    let runtime = Runtime::new().expect("create runtime");

    let polls = Arc::new(AtomicUsize::new(0));
    let handle = runtime
        .spawn_task(CountingFuture {
            polls: polls.clone(),
            target: 3,
        })
        .expect("spawn task");

    assert_eq!(
        handle.recv_timeout(Duration::from_secs(2)).expect("recv"),
        3,
        "future must complete after reaching its target poll count"
    );

    // Deduplication: each self-wake while pending must schedule exactly one
    // additional poll. A naive wake that re-enqueues unconditionally would
    // poll the task more than `target` times.
    assert_eq!(
        polls.load(Ordering::SeqCst),
        3,
        "task must be polled exactly 3 times, no more"
    );
}

#[test]
fn test_task_panic_is_isolated() {
    let runtime = Runtime::new().expect("create runtime");

    let handle = runtime
        .spawn_task(async {
            panic!("async task exploded");
            #[allow(unreachable_code)]
            1u64
        })
        .expect("spawn task");

    match handle.recv_timeout(Duration::from_secs(2)) {
        Err(TaskError::Panic(msg)) => assert!(
            msg.contains("exploded"),
            "panic message must be attached to the handle, got: {msg}"
        ),
        other => panic!("expected TaskError::Panic, got {other:?}"),
    }

    // The executor survives the panic and keeps running unrelated work.
    let ok = runtime
        .spawn_task(async { 9u64 })
        .expect("spawn after panic");
    assert_eq!(ok.recv_timeout(Duration::from_secs(2)).expect("recv"), 9);
}

#[test]
fn test_multiple_tasks_run_concurrently() {
    let runtime = Runtime::new().expect("create runtime");

    let mut handles = Vec::new();
    for i in 0..8u32 {
        handles.push(
            runtime
                .spawn_task(async move { i * i })
                .expect("spawn task"),
        );
    }

    for (i, handle) in handles.iter().enumerate() {
        let want = (i as u32) * (i as u32);
        assert_eq!(
            handle.recv_timeout(Duration::from_secs(2)).expect("recv"),
            want,
            "task {i} returned a wrong value"
        );
    }
}

#[test]
fn test_task_ids_are_unique() {
    use runact::TaskHandle;

    let runtime = Runtime::new().expect("create runtime");

    let h1: TaskHandle<u32> = runtime.spawn_task(async { 1u32 }).expect("spawn");
    let h2: TaskHandle<u32> = runtime.spawn_task(async { 2u32 }).expect("spawn");

    assert_ne!(h1.id(), h2.id(), "each task must get a unique id");
}

#[test]
fn test_many_tasks_stress() {
    let runtime = Runtime::new().expect("create runtime");

    let mut handles = Vec::with_capacity(10_000);
    for i in 0..10_000u32 {
        handles.push(runtime.spawn_task(async move { i }).expect("spawn"));
    }

    for (i, handle) in handles.iter().enumerate() {
        assert_eq!(
            handle.recv_timeout(Duration::from_secs(10)).expect("recv"),
            i as u32,
            "stress task {i} failed"
        );
    }
}

#[test]
fn test_shutdown_returns_and_marks_pending_tasks() {
    let mut runtime = Runtime::new().expect("create runtime");

    // A task that completes quickly; its result stays available after shutdown.
    let done = runtime
        .spawn_task(async { 5u64 })
        .expect("spawn completed task");
    assert_eq!(done.recv_timeout(Duration::from_secs(2)).expect("recv"), 5);

    // A task that will never complete on its own.
    let waker_slot = Arc::new(Mutex::new(None::<Waker>));
    let signal = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicUsize::new(0));
    let pending = runtime
        .spawn_task(ManualFuture {
            waker_slot,
            signal,
            polls: polls.clone(),
            value: 11,
        })
        .expect("spawn pending task");
    wait_until_polled(&polls, 1);

    // Shutdown must return promptly (structured, §15): it must not hang on a
    // task that will never complete.
    let start = Instant::now();
    runtime.shutdown().expect("shutdown");
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "shutdown must not block indefinitely on pending tasks"
    );

    // A never-completing task's handle resolves deterministically to a
    // shutdown error instead of hanging forever.
    match pending.recv() {
        Err(TaskError::ExecutorShutdown) => {}
        other => panic!("expected TaskError::ExecutorShutdown, got {other:?}"),
    }

    // A completed task's result remains retrievable after shutdown.
    assert_eq!(done.try_recv(), Some(Ok(5)));

    // No new work is accepted after shutdown.
    let err = runtime
        .spawn_task(async { 1u32 })
        .expect_err("spawn after shutdown");
    assert!(matches!(err, TaskError::ExecutorShutdown));
}

#[test]
fn test_dropped_handle_detaches_task() {
    let runtime = Runtime::new().expect("create runtime");

    // Dropping the handle must not cancel the task: it runs to completion.
    let polls = Arc::new(AtomicUsize::new(0));
    {
        let _detached = runtime
            .spawn_task(CountingFuture {
                polls: polls.clone(),
                target: 4,
            })
            .expect("spawn task");
    } // handle dropped here

    let deadline = Instant::now() + Duration::from_secs(5);
    while polls.load(Ordering::SeqCst) < 4 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(
        polls.load(Ordering::SeqCst),
        4,
        "dropping the handle must not stop the task"
    );
}
