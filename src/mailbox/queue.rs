use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Condvar};
use crate::actor::ActorId;
use crate::error::RuntimeError;

/// Backpressure policy when mailbox is full.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressurePolicy {
    Reject,
    Block,
    DropLowPriority,
}

/// Mailbox configuration.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MailboxConfig {
    pub capacity: usize,
    pub backpressure: BackpressurePolicy,
}

impl Default for MailboxConfig {
    fn default() -> Self {
        Self {
            capacity: 1000,
            backpressure: BackpressurePolicy::Reject,
        }
    }
}

/// Typed message queue for an actor.
#[allow(dead_code)]
pub struct Mailbox<T> {
    inner: Arc<(Mutex<VecDeque<T>>, Condvar)>,
    config: MailboxConfig,
}

#[allow(dead_code)]
impl<T> Mailbox<T> {
    /// Create a new mailbox.
    pub fn new(config: MailboxConfig) -> Self {
        Self {
            inner: Arc::new((Mutex::new(VecDeque::with_capacity(config.capacity)), Condvar::new())),
            config,
        }
    }

    /// Send a message to the mailbox.
    pub fn send(&self, message: T) -> Result<(), RuntimeError> {
        let (lock, cvar) = &*self.inner;
        let mut queue = lock.lock().map_err(|_| RuntimeError::RuntimeStopped)?;

        if queue.len() >= self.config.capacity {
            match self.config.backpressure {
                BackpressurePolicy::Reject => {
                    return Err(RuntimeError::MailboxFull(ActorId::new(0)));
                }
                BackpressurePolicy::Block => {
                    // Wait for space
                    while queue.len() >= self.config.capacity {
                        queue = cvar.wait(queue).map_err(|_| RuntimeError::RuntimeStopped)?;
                    }
                }
                BackpressurePolicy::DropLowPriority => {
                    queue.pop_front();
                }
            }
        }

        queue.push_back(message);
        cvar.notify_one();
        Ok(())
    }

    /// Receive a message from the mailbox (blocking).
    pub fn receive(&self) -> Option<T> {
        let (lock, cvar) = &*self.inner;
        let mut queue = lock.lock().ok()?;

        loop {
            if let Some(message) = queue.pop_front() {
                return Some(message);
            }
            queue = cvar.wait(queue).ok()?;
        }
    }

    /// Try to receive a message (non-blocking).
    pub fn try_receive(&self) -> Option<T> {
        let (lock, _) = &*self.inner;
        let mut queue = lock.lock().ok()?;
        queue.pop_front()
    }

    /// Get the mailbox length.
    pub fn len(&self) -> usize {
        let (lock, _) = &*self.inner;
        lock.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// Check if the mailbox is empty.
    pub fn is_empty(&self) -> bool {
        let (lock, _) = &*self.inner;
        lock.lock().map(|q| q.is_empty()).unwrap_or(true)
    }
}

impl<T> Clone for Mailbox<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            config: self.config.clone(),
        }
    }
}
