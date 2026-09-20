use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::thread;
use std::time::Duration;

use super::run_queue::RunQueue;

pub struct Scheduler {
    queues: Vec<RunQueue>,
    next_worker: AtomicUsize,
    handles: Vec<thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl Scheduler {
    pub fn new() -> Self {
        let num_workers = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        let stop = Arc::new(AtomicBool::new(false));

        let queues: Vec<RunQueue> = (0..num_workers).map(|_| RunQueue::new()).collect();
        let mut handles = Vec::with_capacity(num_workers);

        for id in 0..num_workers {
            let local_queue = queues[id].clone();
            let steal_targets: Vec<RunQueue> = queues
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != id)
                .map(|(_, q)| q.clone())
                .collect();
            let stop_clone = stop.clone();

            handles.push(thread::spawn(move || {
                Self::worker_loop(local_queue, steal_targets, stop_clone);
            }));
        }

        Self {
            queues,
            next_worker: AtomicUsize::new(0),
            handles,
            stop,
        }
    }

    fn worker_loop(local_queue: RunQueue, steal_targets: Vec<RunQueue>, stop: Arc<AtomicBool>) {
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            // Block-wait for local work (up to 10ms), then try stealing.
            if let Some(task) = local_queue.pop_timeout(Duration::from_millis(10)) {
                task();
                continue;
            }

            let mut stolen = false;
            for target in &steal_targets {
                let stolen_tasks = target.steal_half();
                if !stolen_tasks.is_empty() {
                    for task in stolen_tasks {
                        local_queue.push(task);
                    }
                    stolen = true;
                    break;
                }
            }

            if !stolen {
                // Both local queue and all steal targets are empty.
                // Block for a short interval before re-checking, rather than
                // spinning on `yield_now` (which burns CPU at idle).
                std::thread::park_timeout(std::time::Duration::from_millis(10));
            }
        }
    }

    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let idx = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.queues.len();
        self.queues[idx].push(Box::new(f));
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);

        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }

    pub fn worker_count(&self) -> usize {
        self.queues.len()
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
