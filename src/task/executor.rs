//! The async task executor: worker threads polling runnable futures.
//!
//! Mirrors the compute scheduler's thread-pool shape ([`ComputeScheduler`]),
//! but the unit of work is a [`TaskCell`] whose future is polled until it
//! completes or parks. A live-task registry lets shutdown sweep every
//! outstanding task regardless of whether it is queued, running, or parked.

use super::cell::{ErasedFuture, TaskCell};
use super::error::TaskError;
use super::handle::TaskHandle;
use super::id::TaskId;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Polls runnable tasks on a dedicated pool of worker threads.
pub(crate) struct Executor {
    queue: crossbeam_channel::Sender<Arc<TaskCell>>,
    registry: Arc<Mutex<HashMap<TaskId, Arc<TaskCell>>>>,
    handles: Vec<std::thread::JoinHandle<()>>,
    accept: Arc<AtomicBool>,
}

impl Executor {
    /// Create an executor with one worker per available parallelism.
    pub(crate) fn new() -> Self {
        let (queue, receiver) = crossbeam_channel::unbounded();
        let registry = Arc::new(Mutex::new(HashMap::new()));
        let accept = Arc::new(AtomicBool::new(true));

        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let rx = receiver.clone();
            let registry = registry.clone();
            let accept = accept.clone();
            let handle = std::thread::Builder::new()
                .name("runact-async-worker".to_string())
                .spawn(move || Self::worker_loop(rx, registry, accept))
                .expect("failed to spawn async worker thread");
            handles.push(handle);
        }

        Self {
            queue,
            registry,
            handles,
            accept,
        }
    }

    /// Spawn a future, polled to completion by the worker pool.
    ///
    /// Registration and the acceptance check happen under the registry lock,
    /// so a spawn racing a shutdown either registers and is swept, or fails
    /// with [`TaskError::ExecutorShutdown`] — never orphaned.
    pub(crate) fn spawn<F, T>(&self, future: F) -> Result<TaskHandle<T>, TaskError>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let id = TaskId::new();
        let (result_tx, result_rx) = crossbeam_channel::bounded(1);

        // Type-erase the output: the cell is non-generic, the handle knows T.
        let erased: ErasedFuture =
            Box::pin(async move { Box::new(future.await) as Box<dyn std::any::Any + Send> });

        let cell = Arc::new(TaskCell::new(id, erased, result_tx, self.queue.clone()));

        let mut registry = self.registry.lock().expect("task registry mutex poisoned");
        if !self.accept.load(Ordering::Acquire) {
            return Err(TaskError::ExecutorShutdown);
        }
        registry.insert(id, cell.clone());
        drop(registry);

        cell.enqueue();

        Ok(TaskHandle::new(id, result_rx))
    }

    /// Shut the executor down.
    ///
    /// Rejects new work, sweeps every live task so pending handles resolve
    /// to [`TaskError::ExecutorShutdown`], then joins the workers.
    pub(crate) fn shutdown(&mut self) {
        self.accept.store(false, Ordering::Release);

        let live: Vec<Arc<TaskCell>> = {
            let mut registry = self.registry.lock().expect("task registry mutex poisoned");
            registry.drain().map(|(_, cell)| cell).collect()
        };
        for cell in &live {
            cell.sweep();
        }

        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }

    fn worker_loop(
        receiver: crossbeam_channel::Receiver<Arc<TaskCell>>,
        registry: Arc<Mutex<HashMap<TaskId, Arc<TaskCell>>>>,
        accept: Arc<AtomicBool>,
    ) {
        loop {
            if !accept.load(Ordering::Relaxed) {
                break;
            }
            match receiver.recv_timeout(std::time::Duration::from_millis(10)) {
                Ok(cell) => {
                    if cell.run_once() {
                        registry
                            .lock()
                            .expect("task registry mutex poisoned")
                            .remove(&cell.id());
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            }
        }
    }
}
