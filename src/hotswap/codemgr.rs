use crate::error::RuntimeError;
use crate::hotswap::loader::DynamicLibrary;
use crate::hotswap::loader::LibraryLoader;
use crate::hotswap::types::ActorVTable;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// A loaded code path for a module — contains the dynamic library and its vtable.
#[derive(Clone)]
pub struct CodePath {
    pub version: String,
    pub library: Arc<DynamicLibrary>,
    pub vtable: Arc<ActorVTable>,
}

/// Manages all versions of dynamically loaded actor modules.
///
/// This is the heart of hot code loading. Each module can have multiple
/// versions loaded, but only one is "current" — new processes spawned for
/// that module type always use the current version.
pub struct CodeManager {
    /// Module name -> list of versions (latest/current version at end).
    code_paths: HashMap<String, Vec<CodePath>>,
    /// Module name -> index into code_paths list of current version.
    current_versions: HashMap<String, usize>,
}

impl CodeManager {
    pub fn new() -> Self {
        Self {
            code_paths: HashMap::new(),
            current_versions: HashMap::new(),
        }
    }

    /// Load a new version of an actor module from a dynamic library.
    pub fn load_new_version<P: AsRef<Path>>(
        &mut self,
        module_name: &str,
        library_path: P,
    ) -> Result<String, RuntimeError> {
        let library = LibraryLoader::load(library_path.as_ref())?;
        let vtable = library.get_vtable()?;

        let old_version = {
            let idx = self.current_versions.get(module_name).copied();
            let versions = self.code_paths.get(module_name);
            idx.zip(versions)
                .and_then(|(i, v)| v.get(i))
                .map(|cp| cp.version.clone())
                .unwrap_or_else(|| "none".to_string())
        };

        let new_version = vtable.version.clone();

        let code_path = CodePath {
            version: new_version.clone(),
            library,
            vtable: Arc::new(vtable),
        };

        let versions = self.code_paths.entry(module_name.to_string()).or_default();
        versions.push(code_path);

        // Make this the current version for new processes
        self.current_versions
            .insert(module_name.to_string(), versions.len() - 1);

        tracing::info!(
            module = module_name,
            new_version = %new_version,
            old_version = %old_version,
            "hot code load: new processes will use new version"
        );

        Ok(old_version)
    }

    /// Get the vtable for the CURRENT version of a module.
    pub fn get_current_vtable(&self, module_name: &str) -> Option<Arc<ActorVTable>> {
        let idx = self.current_versions.get(module_name)?;
        let versions = self.code_paths.get(module_name)?;
        versions.get(*idx).map(|cp| cp.vtable.clone())
    }

    /// Get a specific version's vtable.
    pub fn get_version_vtable(&self, module_name: &str, version: &str) -> Option<Arc<ActorVTable>> {
        self.code_paths.get(module_name).and_then(|versions| {
            versions
                .iter()
                .find(|cp| cp.version == version)
                .map(|cp| cp.vtable.clone())
        })
    }

    /// List all loaded versions for a module.
    pub fn list_versions(&self, module_name: &str) -> Vec<String> {
        self.code_paths
            .get(module_name)
            .map(|versions| versions.iter().map(|cp| cp.version.clone()).collect())
            .unwrap_or_default()
    }

    /// Get the current version string for a module.
    pub fn current_version(&self, module_name: &str) -> Option<String> {
        let idx = self.current_versions.get(module_name)?;
        let versions = self.code_paths.get(module_name)?;
        versions.get(*idx).map(|cp| cp.version.clone())
    }

    /// Check if a module has been loaded.
    pub fn has_module(&self, module_name: &str) -> bool {
        self.code_paths.contains_key(module_name)
    }

    /// Get the number of loaded versions for a module.
    pub fn version_count(&self, module_name: &str) -> usize {
        self.code_paths
            .get(module_name)
            .map(|versions| versions.len())
            .unwrap_or(0)
    }
}

impl Default for CodeManager {
    fn default() -> Self {
        Self::new()
    }
}
