use crate::compute::task::{ComputeError, ComputeResult, Task, TaskId};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Thread pool for executing CPU-intensive tasks off the actor scheduler.
///
/// Workers pull tasks from a shared unbounded queue and execute them,
/// catching panics to isolate failures. Each worker checks the task's
/// cancellation flag before and after execution.
pub struct ComputeScheduler {
    sender: Option<crossbeam_channel::Sender<Task>>,
    handles: Vec<thread::JoinHandle<()>>,
    _config: ComputeConfig,
    stop: Arc<AtomicBool>,
}

/// Configuration for the compute scheduler.
#[derive(Debug, Clone)]
pub struct ComputeConfig {
    /// Number of worker threads. Defaults to available parallelism.
    pub max_workers: usize,
    /// Maximum tasks in the unbounded queue (soft limit).
    pub queue_capacity: usize,
    /// Optional per-task timeout. Not yet enforced in the worker loop.
    pub task_timeout: Option<std::time::Duration>,
}

impl Default for ComputeConfig {
    fn default() -> Self {
        Self {
            max_workers: thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            queue_capacity: 1024,
            task_timeout: None,
        }
    }
}

impl ComputeScheduler {
    /// Create a new compute scheduler with the given configuration.
    ///
    /// Spawns `max_workers` threads, each running a worker loop that pulls
    /// tasks from a shared channel.
    pub fn new(config: ComputeConfig) -> Result<Self, ComputeError> {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let mut handles = Vec::with_capacity(config.max_workers);
        let stop = Arc::new(AtomicBool::new(false));

        for _ in 0..config.max_workers {
            let rx = receiver.clone();
            let stop_clone = stop.clone();
            let handle = thread::spawn(move || {
                Self::worker_loop(rx, stop_clone);
            });
            handles.push(handle);
        }

        Ok(Self {
            sender: Some(sender),
            handles,
            _config: config,
            stop,
        })
    }

    fn worker_loop(receiver: crossbeam_channel::Receiver<Task>, stop: Arc<AtomicBool>) {
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            match receiver.recv_timeout(std::time::Duration::from_millis(10)) {
                Ok(task) => {
                    if task.cancelled.load(Ordering::Relaxed) {
                        let _ = task.result_sender.send(ComputeResult::Cancelled);
                        continue;
                    }

                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (task.job)()));

                    if task.cancelled.load(Ordering::Relaxed) {
                        let _ = task.result_sender.send(ComputeResult::Cancelled);
                        continue;
                    }

                    let compute_result = match result {
                        Ok(value) => ComputeResult::Ok(value),
                        Err(panic) => {
                            let msg = if let Some(s) = panic.downcast_ref::<&str>() {
                                s.to_string()
                            } else if let Some(s) = panic.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                "Unknown panic".to_string()
                            };
                            ComputeResult::Panic(msg)
                        }
                    };

                    let _ = task.result_sender.send(compute_result);
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    /// Submit a CPU-intensive job to the compute pool.
    ///
    /// Returns a [`crate::ComputeHandle`] that can be polled for the result.
    /// Panics inside the job are caught and surfaced as [`ComputeError::WorkerPanic`].
    pub fn spawn<F, T>(
        &self,
        job: F,
    ) -> Result<crate::compute::handle::ComputeHandle<T>, ComputeError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (result_sender, result_receiver) = crossbeam_channel::bounded(1);
        let cancelled = Arc::new(AtomicBool::new(false));

        let task = Task {
            id: TaskId::new(),
            job: Box::new(move || Box::new(job()) as Box<dyn std::any::Any + Send>),
            result_sender,
            created_at: std::time::Instant::now(),
            cancelled: cancelled.clone(),
        };

        match self.sender.as_ref().unwrap().send(task) {
            Ok(()) => Ok(crate::compute::handle::ComputeHandle::new(
                result_receiver,
                cancelled,
            )),
            Err(_) => Err(ComputeError::SchedulerShutdown),
        }
    }

    pub fn sender(&self) -> Option<crossbeam_channel::Sender<Task>> {
        self.sender.clone()
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.sender.take();
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}
