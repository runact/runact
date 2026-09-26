use crate::error::RuntimeError;
use crate::hotswap::types::ActorVTable;
use libloading::Library;
use std::ffi::CStr;
use std::path::Path;
use std::sync::Arc;

/// Wrapper around a dynamically loaded library with safe cleanup.
pub struct DynamicLibrary {
    lib: Library,
    #[allow(dead_code)]
    path: std::path::PathBuf,
}

impl DynamicLibrary {
    pub fn load(path: &Path) -> Result<Self, RuntimeError> {
        let lib = unsafe { Library::new(path) }.map_err(|_| RuntimeError::RuntimeStopped)?;
        Ok(Self {
            lib,
            path: path.to_path_buf(),
        })
    }

    /// Retrieve the vtable symbol exported by the dynamic library.
    pub fn get_vtable(&self) -> Result<ActorVTable, RuntimeError> {
        unsafe {
            let version_fn: libloading::Symbol<extern "C" fn() -> *const i8> = self
                .lib
                .get(b"runact_actor_version")
                .map_err(|_| RuntimeError::RuntimeStopped)?;
            let version_ptr = version_fn();
            let version_cstr = CStr::from_ptr(version_ptr);
            let version = version_cstr
                .to_str()
                .map_err(|_| RuntimeError::RuntimeStopped)?
                .to_string();

            let type_name_fn: libloading::Symbol<extern "C" fn() -> *const i8> = self
                .lib
                .get(b"runact_actor_type_name")
                .map_err(|_| RuntimeError::RuntimeStopped)?;
            let type_name_ptr = type_name_fn();
            let type_name_cstr = CStr::from_ptr(type_name_ptr);
            let type_name = type_name_cstr
                .to_str()
                .map_err(|_| RuntimeError::RuntimeStopped)?
                .to_string();

            let vtable_ptr_fn: libloading::Symbol<extern "C" fn() -> *const u64> = self
                .lib
                .get(b"runact_actor_vtable_ptr")
                .map_err(|_| RuntimeError::RuntimeStopped)?;
            let ptr = vtable_ptr_fn() as *const ActorVTable;
            let vtable = ptr.read();

            Ok(ActorVTable {
                version,
                type_name,
                handle_message_fn: vtable.handle_message_fn,
                serialize_fn: vtable.serialize_fn,
                deserialize_fn: vtable.deserialize_fn,
                migrate_fn: vtable.migrate_fn,
                drop_fn: vtable.drop_fn,
            })
        }
    }
}

impl Drop for DynamicLibrary {
    fn drop(&mut self) {
        // Library is automatically closed by libloading on drop
    }
}

/// Loader for dynamic libraries with error handling.
pub struct LibraryLoader;

impl LibraryLoader {
    pub fn load(path: &Path) -> Result<Arc<DynamicLibrary>, RuntimeError> {
        let lib = DynamicLibrary::load(path)?;
        Ok(Arc::new(lib))
    }
}
