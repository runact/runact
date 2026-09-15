use std::time::Duration;

/// Child specification.
#[derive(Debug, Clone)]
pub struct ChildSpec {
    pub name: String,
    pub restart_policy: RestartPolicy,
    pub shutdown_timeout: Duration,
}

impl ChildSpec {
    /// Create a new child specification.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            restart_policy: RestartPolicy::Permanent,
            shutdown_timeout: Duration::from_secs(5),
        }
    }

    /// Set the restart policy.
    pub fn restart_policy(mut self, policy: RestartPolicy) -> Self {
        self.restart_policy = policy;
        self
    }

    /// Set the shutdown timeout.
    pub fn shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }
}

/// Restart policy for a child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    /// Always restart.
    Permanent,
    /// Never restart.
    Temporary,
    /// Restart only on abnormal exit.
    Transient,
}
