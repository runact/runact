use crate::error::RuntimeError;
use std::collections::HashMap;

/// Type alias for a state migration function.
pub type MigrationFn = fn(Vec<u8>) -> Vec<u8>;

/// Registry for state migration functions between module versions.
pub struct MigrationRegistry {
    migrations: HashMap<(String, String, String), MigrationFn>,
}

impl MigrationRegistry {
    pub fn new() -> Self {
        Self {
            migrations: HashMap::new(),
        }
    }

    /// Register a migration function from one version to another.
    pub fn register(&mut self, module: &str, from: &str, to: &str, f: MigrationFn) {
        self.migrations
            .insert((module.to_string(), from.to_string(), to.to_string()), f);
    }

    /// Migrate state from one version to another, chaining intermediate migrations.
    pub fn migrate(
        &self,
        module: &str,
        from: &str,
        to: &str,
        state: Vec<u8>,
    ) -> Result<Vec<u8>, RuntimeError> {
        if from == to {
            return Ok(state);
        }

        // Try direct migration first
        if let Some(&f) =
            self.migrations
                .get(&(module.to_string(), from.to_string(), to.to_string()))
        {
            return Ok(f(state));
        }

        // Chain migrations
        let all_versions: Vec<String> = self
            .migrations
            .keys()
            .filter(|(m, f, _)| m == module && f == from)
            .map(|(_, _, t)| t.clone())
            .collect();

        let mut current_state = state;
        let mut current_version = from;

        for target in &all_versions {
            if target <= &current_version.to_string() || target > &to.to_string() {
                continue;
            }
            if let Some(&f) = self.migrations.get(&(
                module.to_string(),
                current_version.to_string(),
                target.clone(),
            )) {
                current_state = f(current_state);
                current_version = target;
                if current_version == to {
                    break;
                }
            }
        }

        if current_version == to {
            Ok(current_state)
        } else {
            Err(RuntimeError::RuntimeStopped)
        }
    }
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for actors that implement version-aware state migration.
pub trait StateMigration: Sized {
    /// Migrate state from a previous version.
    fn migrate_from(from_version: &str, state: Vec<u8>) -> Result<Self, RuntimeError>
    where
        Self: Sized;
}
