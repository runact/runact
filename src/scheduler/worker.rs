use super::run_queue::RunQueue;

#[allow(dead_code)]
pub struct Worker {
    id: usize,
    queue: RunQueue,
}

#[allow(dead_code)]
impl Worker {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            queue: RunQueue::new(),
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn queue(&self) -> &RunQueue {
        &self.queue
    }

    pub fn steal_from(&self, victim: &RunQueue) -> usize {
        let stolen = victim.steal_half();
        let count = stolen.len();
        for task in stolen {
            self.queue.push(task);
        }
        count
    }
}
