use crate::actor::ActorId;
use std::time::{Duration, Instant};

/// Timer ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);

impl TimerId {
    /// Create a new TimerId.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for TimerId {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer configuration.
#[derive(Debug)]
pub struct TimerConfig {
    pub id: TimerId,
    pub actor_id: ActorId,
    pub scheduled_at: Instant,
    pub message: Box<dyn std::any::Any + Send>,
    pub interval: Option<Duration>,
}

/// Timer — scheduled message delivery.
pub struct Timer {
    id: TimerId,
    actor_id: ActorId,
    scheduled_at: Instant,
    _message: Box<dyn std::any::Any + Send>,
    interval: Option<Duration>,
}

impl Timer {
    /// Create a one-shot timer.
    pub fn after(
        duration: Duration,
        actor_id: ActorId,
        message: impl Into<Box<dyn std::any::Any + Send>>,
    ) -> Self {
        Self {
            id: TimerId::new(),
            actor_id,
            scheduled_at: Instant::now() + duration,
            _message: message.into(),
            interval: None,
        }
    }

    /// Create a periodic timer.
    pub fn every(
        interval: Duration,
        actor_id: ActorId,
        message: impl Into<Box<dyn std::any::Any + Send>>,
    ) -> Self {
        Self {
            id: TimerId::new(),
            actor_id,
            scheduled_at: Instant::now() + interval,
            _message: message.into(),
            interval: Some(interval),
        }
    }

    /// Get the timer's ID.
    pub fn id(&self) -> TimerId {
        self.id
    }

    /// Get the target actor's ID.
    pub fn actor_id(&self) -> ActorId {
        self.actor_id
    }

    /// Get the scheduled time.
    pub fn scheduled_at(&self) -> Instant {
        self.scheduled_at
    }

    /// Check if the timer is due.
    pub fn is_due(&self) -> bool {
        Instant::now() >= self.scheduled_at
    }

    /// Check if the timer is periodic.
    pub fn is_periodic(&self) -> bool {
        self.interval.is_some()
    }

    /// Reschedule a periodic timer.
    pub fn reschedule(&mut self) {
        if let Some(interval) = self.interval {
            self.scheduled_at = Instant::now() + interval;
        }
    }
}
