use std::any::Any;
use std::fmt;

/// Trait for resource handles that can be stored in a [`ResourceRegistry`](crate::resource::ResourceRegistry).
///
/// Implement this trait on your resource types to make them type-safely
/// retrievable from the registry.
pub trait ResourceHandle: Any + Send + Sync + fmt::Debug + 'static {
    /// Returns a human-readable name for the resource type (e.g., `"FileHandle"`).
    fn resource_type(&self) -> &str;

    /// Upcast to `&dyn Any` for type-erased storage.
    fn as_any(&self) -> &dyn Any;
}
