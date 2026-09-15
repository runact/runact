use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

type Task = Box<dyn FnOnce() + Send>;

#[derive(Clone)]
pub struct RunQueue {
    queue: Arc<Mutex<VecDeque<Task>>>,
}

impl RunQueue {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn push(&self, task: Box<dyn FnOnce() + Send>) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(task);
    }

    pub fn pop(&self) -> Option<Box<dyn FnOnce() + Send>> {
        let mut queue = self.queue.lock().unwrap();
        queue.pop_front()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        let queue = self.queue.lock().unwrap();
        queue.is_empty()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        let queue = self.queue.lock().unwrap();
        queue.len()
    }

    pub(crate) fn steal_half(&self) -> Vec<Box<dyn FnOnce() + Send>> {
        let mut queue = self.queue.lock().unwrap();
        let len = queue.len();
        if len <= 1 {
            return Vec::new();
        }
        let steal_count = len / 2;
        queue.drain(..steal_count).collect()
    }
}

impl Default for RunQueue {
    fn default() -> Self {
        Self::new()
    }
}
