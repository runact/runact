use super::capability::Capability;
use super::handle::ResourceHandle;
use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::RwLock;

/// Type-safe registry for resource capabilities.
#[must_use]
pub struct ResourceRegistry {
    resources: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl ResourceRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            resources: RwLock::new(HashMap::new()),
        }
    }

    /// Register a capability in the registry.
    ///
    /// If a capability of the same handle type already exists, it is replaced.
    pub fn register<H: ResourceHandle + 'static>(&self, capability: Capability<H>) {
        let mut resources = self.resources.write().unwrap();
        resources.insert(TypeId::of::<H>(), Box::new(capability));
    }

    /// Retrieve a capability by handle type. Returns `None` if not registered.
    pub fn get<H: ResourceHandle + 'static>(&self) -> Option<Capability<H>> {
        let resources = self.resources.read().unwrap();
        resources
            .get(&TypeId::of::<H>())
            .and_then(|any| any.downcast_ref::<Capability<H>>())
            .cloned()
    }

    /// Remove and return a capability by handle type.
    pub fn remove<H: ResourceHandle + 'static>(&self) -> Option<Capability<H>> {
        let mut resources = self.resources.write().unwrap();
        resources.remove(&TypeId::of::<H>()).and_then(|any| {
            let boxed = any.downcast::<Capability<H>>().ok()?;
            Some(*boxed)
        })
    }

    /// Returns `true` if a capability of the given handle type is registered.
    pub fn has<H: ResourceHandle + 'static>(&self) -> bool {
        let resources = self.resources.read().unwrap();
        resources.contains_key(&TypeId::of::<H>())
    }
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
