use std::time::Duration;

/// Strategy for restarting failed child actors.
///
/// Currently only `OneForOne` is supported: each failed child is restarted
/// independently, without affecting siblings.
#[derive(Debug, Clone)]
pub enum RestartStrategy {
    /// Restart only the failed child actor.
    OneForOne {
        /// Maximum number of restarts allowed within the `within` window.
        max_restarts: usize,
        /// Time window for counting restarts.
        within: Duration,
        /// Initial backoff between restarts. Doubles each restart, capped at 30s.
        base_backoff: Duration,
    },
}

impl Default for RestartStrategy {
    fn default() -> Self {
        Self::OneForOne {
            max_restarts: 5,
            within: Duration::from_secs(60),
            base_backoff: Duration::from_millis(100),
        }
    }
}
