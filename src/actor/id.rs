use serde::{Deserialize, Serialize};

/// Stable, unique identifier for an actor.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ActorId(u64);

impl ActorId {
    /// Create a new ActorId.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw identifier.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for ActorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Actor({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actor_id_equality() {
        let a = ActorId::new(1);
        let b = ActorId::new(1);
        let c = ActorId::new(2);

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_actor_id_ordering() {
        let a = ActorId::new(1);
        let b = ActorId::new(2);

        assert!(a < b);
    }
}
