//! The shared task cell: state machine, future storage, and poll loop.

use crate::task::error::TaskResult;
use crate::task::id::TaskId;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll, Wake, Waker};

/// The boxed output of an erased task future: the user's value, type-erased so
/// the cell stays non-generic.
pub(super) type ErasedOutput = Box<dyn std::any::Any + Send>;
/// A heap-pinned, type-erased future the worker poll loop can drive.
pub(super) type ErasedFuture = Pin<Box<dyn Future<Output = ErasedOutput> + Send>>;

/// Task state bits stored in [`TaskCell::state`].
///
/// A task is in exactly the "coarsest" state that applies:
/// - `IDLE` — parked; not queued, not being polled. A future wake must enqueue it.
/// - `SCHEDULED` — queued (or owed a run). Exactly one queue entry exists per
///   transition into (or retention of) this bit.
/// - `RUNNING` — a worker is currently polling it. Wakes during a poll *set*
///   `SCHEDULED` without enqueueing; the worker re-enqueues when it finishes.
/// - `COMPLETED` — terminal. Either a result was delivered or shutdown swept it.
///
/// `SCHEDULED` is deliberately kept live while `RUNNING` so a wake racing a
/// poll is never lost: whoever clears `RUNNING` last re-enqueues if `SCHEDULED`
/// is still set. The property is "exactly one worker will ever poll between two
/// successive `IDLE` states".
const IDLE: u64 = 0;
const SCHEDULED: u64 = 1 << 0;
const RUNNING: u64 = 1 << 1;
const COMPLETED: u64 = 1 << 2;

/// The mutable half of a task: its future and the channel the worker uses to
/// deliver terminal outcomes to the owning handle.
pub(super) struct TaskCell {
    id: TaskId,
    state: AtomicU64,
    /// The user future (type-erased to `Box<dyn Any + Send>` output so the
    /// cell itself is non-generic). Present until the task completes.
    future: Mutex<Option<ErasedFuture>>,
    /// Delivers [`TaskResult`] to the handle. Held by exactly the cell so a
    /// dropped cell disconnects waiting handles.
    result_sender: crossbeam_channel::Sender<TaskResult>,
    /// Runnable-queue sender used by the waker and by a worker re-enqueueing a
    /// task that was woken while running.
    queue_sender: crossbeam_channel::Sender<Arc<TaskCell>>,
}

impl TaskCell {
    pub(super) fn new(
        id: TaskId,
        future: ErasedFuture,
        result_sender: crossbeam_channel::Sender<TaskResult>,
        queue_sender: crossbeam_channel::Sender<Arc<TaskCell>>,
    ) -> Self {
        Self {
            id,
            // A freshly spawned task starts scheduled: its first queue entry
            // already exists, so `SCHEDULED` must be set before any poll.
            state: AtomicU64::new(SCHEDULED),
            future: Mutex::new(Some(future)),
            result_sender,
            queue_sender,
        }
    }

    pub(super) fn id(&self) -> TaskId {
        self.id
    }

    /// Signal that the task should be polled soon.
    ///
    /// Enqueues exactly once per `IDLE → SCHEDULED` transition. If the task is
    /// already scheduled, the call is a no-op (wake deduplication). If it is
    /// running, the worker re-enqueues after finishing its poll, so no push
    /// happens here — this is what prevents both a lost wakeup and a double
    /// enqueue.
    pub(super) fn wake(self: &Arc<Self>) {
        let prev = self.state.fetch_or(SCHEDULED, Ordering::AcqRel);
        if prev == IDLE {
            // We performed the IDLE → SCHEDULED transition; publish the queue entry.
            self.enqueue();
        }
    }

    /// Poll the task once. Claims the run (`SCHEDULED → RUNNING`), polls the
    /// user future, then either completes it or releases the bit.
    ///
    /// Returns `true` when the task reached a terminal state (completion,
    /// panic, or shutdown sweep) and must be deregistered, `false` when it
    /// parked again as `IDLE` or was re-enqueued.
    ///
    /// Panics inside the user future are caught and surfaced as
    /// [`TaskError::Panic`](crate::TaskError), so a misbehaving task never
    /// kills a worker.
    ///
    /// # Wake-safety
    ///
    /// The `RUNNING` bit is what lets a parked task go fully idle: on a
    /// `Pending` poll the worker CASes `RUNNING → IDLE` and enqueues again
    /// *only* if a concurrent wake set `SCHEDULED` during the poll. A wake on
    /// an idle task enqueues it directly; a wake on a running task sets
    /// `SCHEDULED` and lets the worker publish the re-enqueue. Exactly one
    /// queue entry exists per `IDLE → (SCHEDULED or RUNNING) → IDLE` cycle.
    pub(super) fn run_once(self: &Arc<Self>) -> bool {
        // Claim the run: SCHEDULED → RUNNING. A sweep may have completed the
        // task while it sat in the queue; bail in that case.
        loop {
            let state = self.state.load(Ordering::Acquire);
            if state & COMPLETED != 0 {
                return true;
            }
            let claimed = (state | RUNNING) & !SCHEDULED;
            match self.state.compare_exchange_weak(
                state,
                claimed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }

        let waker = Waker::from(Arc::new(TaskWaker {
            cell: Arc::downgrade(self),
        }));
        let mut cx = Context::from_waker(&waker);

        let mut future_guard = self.future.lock().expect("task future mutex poisoned");
        let result = match future_guard.as_mut() {
            Some(future) => catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(&mut cx))),
            // The future is already gone: the task completed and the worker
            // that finished it released the state. Nothing further to do.
            None => return true,
        };

        match result {
            Ok(Poll::Ready(output)) => {
                future_guard.take();
                drop(future_guard);
                self.complete(TaskResult::Ok(output));
                true
            }
            Err(panic_payload) => {
                future_guard.take();
                drop(future_guard);
                let msg = panic_message(&*panic_payload);
                self.complete(TaskResult::Panic(msg));
                true
            }
            Ok(Poll::Pending) => {
                drop(future_guard);
                // Race with a wake that fired during our poll: the waker set
                // `SCHEDULED` without enqueueing, expecting us to publish the
                // queue entry. Clear `RUNNING`; if `SCHEDULED` is still set,
                // re-enqueue. If the sweep transitioned to `COMPLETED` while we
                // polled, give up — the handle already got the shutdown result.
                loop {
                    let state = self.state.load(Ordering::Acquire);
                    if state & COMPLETED != 0 {
                        return true;
                    }
                    let next = state & !RUNNING;
                    match self.state.compare_exchange_weak(
                        state,
                        next,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => {
                            if next & SCHEDULED != 0 {
                                self.enqueue();
                            }
                            return false;
                        }
                        Err(_) => continue,
                    }
                }
            }
        }
    }

    /// Transition to `COMPLETED` and deliver the terminal result exactly once.
    ///
    /// If a concurrent sweep already completed the task, the delivered result
    /// wins and this call does nothing.
    fn complete(&self, result: TaskResult) {
        loop {
            let state = self.state.load(Ordering::Acquire);
            if state & COMPLETED != 0 {
                return;
            }
            match self.state.compare_exchange_weak(
                state,
                COMPLETED,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let _ = self.result_sender.try_send(result);
                    return;
                }
                Err(_) => continue,
            }
        }
    }

    /// Shutdown sweep: mark the task completed (if it is not already) and
    /// deliver [`TaskError::ExecutorShutdown`] to any waiting handle.
    ///
    /// Safe to race with a worker: only one of the worker's `complete` and this
    /// sweep can win the `COMPLETED` transition, so exactly one result is ever
    /// delivered.
    pub(super) fn sweep(&self) {
        loop {
            let state = self.state.load(Ordering::Acquire);
            if state & COMPLETED != 0 {
                return;
            }
            match self.state.compare_exchange_weak(
                state,
                COMPLETED,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let _ = self.result_sender.try_send(TaskResult::Shutdown);
                    return;
                }
                Err(_) => continue,
            }
        }
    }

    /// Push this cell onto the runnable queue.
    pub(super) fn enqueue(self: &Arc<Self>) {
        let _ = self.queue_sender.send(self.clone());
    }
}

/// Extracts the panic message string from a `catch_unwind` payload, mirroring
/// the compute scheduler's behavior.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic".to_string()
    }
}

/// The waker handed to the user future. Holds a `Weak` reference so a completed
/// task is never kept alive by a waker the future (or the caller) cloned.
struct TaskWaker {
    cell: std::sync::Weak<TaskCell>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if let Some(cell) = self.cell.upgrade() {
            cell.wake();
        }
    }
}
