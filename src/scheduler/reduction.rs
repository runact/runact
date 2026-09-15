use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum reductions before preemption.
pub const MAX_REDUCTIONS: u64 = 4000;

/// ReductionCounter — prevents starvation.
pub struct ReductionCounter {
    count: AtomicU64,
}

impl ReductionCounter {
    /// Create a new reduction counter.
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
        }
    }

    /// Increment the counter and return the new value.
    pub fn increment(&self) -> u64 {
        self.count.fetch_add(1, Ordering::Relaxed)
    }

    /// Reset the counter to zero.
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
    }

    /// Check if preemption should occur.
    #[allow(dead_code)]
    pub fn should_preempt(&self) -> bool {
        self.count.load(Ordering::Relaxed) >= MAX_REDUCTIONS
    }
}

impl Default for ReductionCounter {
    fn default() -> Self {
        Self::new()
    }
}
