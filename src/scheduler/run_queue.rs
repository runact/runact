use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

type Task = Box<dyn FnOnce() + Send>;

#[derive(Clone)]
pub struct RunQueue {
    queue: Arc<Inner>,
}

struct Inner {
    deque: Mutex<VecDeque<Task>>,
    not_empty: Condvar,
}

impl RunQueue {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Inner {
                deque: Mutex::new(VecDeque::new()),
                not_empty: Condvar::new(),
            }),
        }
    }

    pub fn push(&self, task: Box<dyn FnOnce() + Send>) {
        let mut deque = self.queue.deque.lock().expect("run_queue mutex poisoned");
        deque.push_back(task);
        self.queue.not_empty.notify_one();
    }

    /// Block-wait for a task to arrive on this queue, up to `timeout`.
    /// Returns `None` on timeout.
    pub fn pop_timeout(&self, timeout: std::time::Duration) -> Option<Box<dyn FnOnce() + Send>> {
        let mut deque = self.queue.deque.lock().expect("run_queue mutex poisoned");
        loop {
            if let Some(task) = deque.pop_front() {
                return Some(task);
            }
            let (guard, result) = self
                .queue
                .not_empty
                .wait_timeout(deque, timeout)
                .expect("run_queue condvar poisoned");
            deque = guard;
            if result.timed_out() {
                // Timed out; final drain attempt.
                return deque.pop_front();
            }
            // Notified — loop and try to pop again.
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        let deque = self.queue.deque.lock().expect("run_queue mutex poisoned");
        deque.is_empty()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        let deque = self.queue.deque.lock().expect("run_queue mutex poisoned");
        deque.len()
    }

    pub(crate) fn steal_half(&self) -> Vec<Box<dyn FnOnce() + Send>> {
        let mut deque = self.queue.deque.lock().expect("run_queue mutex poisoned");
        let len = deque.len();
        if len <= 1 {
            return Vec::new();
        }
        let steal_count = len / 2;
        deque.drain(..steal_count).collect()
    }
}

impl Default for RunQueue {
    fn default() -> Self {
        Self::new()
    }
}
