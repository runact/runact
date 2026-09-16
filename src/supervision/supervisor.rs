use crate::actor::ActorId;
use crate::supervision::child::ChildSpec;
use crate::supervision::strategy::RestartStrategy;
use std::collections::HashMap;

/// Supervisor — manages child actor lifecycle.
pub struct Supervisor {
    id: ActorId,
    strategy: RestartStrategy,
    children: Vec<ChildSpec>,
    restart_counts: HashMap<ActorId, usize>,
}

impl Supervisor {
    /// Create a new supervisor.
    pub fn new(id: ActorId, strategy: RestartStrategy) -> Self {
        Self {
            id,
            strategy,
            children: Vec::new(),
            restart_counts: HashMap::new(),
        }
    }

    /// Add a child specification.
    pub fn child(&mut self, spec: ChildSpec) -> &mut Self {
        self.children.push(spec);
        self
    }

    /// Get the supervisor's ID.
    pub fn id(&self) -> ActorId {
        self.id
    }

    /// Get the restart strategy.
    pub fn strategy(&self) -> &RestartStrategy {
        &self.strategy
    }

    /// Get the children.
    pub fn children(&self) -> &[ChildSpec] {
        &self.children
    }

    /// Handle a child failure.
    pub fn handle_child_failure(&mut self, child_id: ActorId) -> bool {
        let count = self.restart_counts.entry(child_id).or_insert(0);
        *count += 1;

        // Check if we should restart
        match &self.strategy {
            RestartStrategy::OneForOne { max_restarts, .. } => {
                if *count > *max_restarts {
                    tracing::error!(
                        child_id = %child_id,
                        restart_count = *count,
                        "Too many restarts, escalating"
                    );
                    false
                } else {
                    tracing::warn!(
                        child_id = %child_id,
                        restart_count = *count,
                        "Restarting actor"
                    );
                    true
                }
            }
        }
    }
}
