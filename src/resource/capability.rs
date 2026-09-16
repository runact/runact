use super::handle::ResourceHandle;
use crate::actor::ActorId;
use std::sync::Arc;

/// Typed, owner-tracked wrapper around a resource handle.
#[must_use]
pub struct Capability<H: ResourceHandle> {
    owner: ActorId,
    handle: Arc<H>,
}

impl<H: ResourceHandle> Capability<H> {
    /// Create a new capability wrapping a resource handle with its owning actor.
    pub fn new(owner: ActorId, handle: H) -> Self {
        Self {
            owner,
            handle: Arc::new(handle),
        }
    }

    /// Returns the actor ID of the capability's owner.
    pub fn owner(&self) -> ActorId {
        self.owner
    }

    /// Borrow the underlying resource handle.
    pub fn handle(&self) -> &H {
        &self.handle
    }

    /// Consume the capability and return the shared handle.
    pub fn into_handle(self) -> Arc<H> {
        self.handle
    }
}

impl<H: ResourceHandle> Clone for Capability<H> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner,
            handle: Arc::clone(&self.handle),
        }
    }
}
