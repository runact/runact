use crate::error::RuntimeError;
use crate::hotswap::codemgr::CodeManager;
use crate::hotswap::types::{ActorSpec, ActorVTable};
use crate::runtime::RuntimeSender;
use std::sync::Arc;
use std::sync::Mutex;

/// Factory for spawning actors with hot code loading support.
///
/// This is the core mechanism that makes Pattern 2 work: when a supervisor
/// spawns a child actor per work item, the factory always uses the CURRENT
/// version of the module — so new child actors automatically run the latest code.
///
/// The factory holds a clonable RuntimeSender so it can spawn actors from
/// external threads (e.g. supervisor loops) without holding a &Runtime borrow.
pub struct ActorFactory {
    #[allow(dead_code)]
    code_manager: Arc<Mutex<CodeManager>>,
    runtime_sender: RuntimeSender,
}

impl ActorFactory {
    pub fn new(code_manager: Arc<Mutex<CodeManager>>, runtime_sender: RuntimeSender) -> Self {
        Self {
            code_manager,
            runtime_sender,
        }
    }

    /// Prepare a dynamic spawn by retrieving the CURRENT version's vtable.
    ///
    /// This is the key function for "new processes use new code":
    /// it always retrieves the latest version's vtable from the CodeManager.
    pub fn prepare_dynamic_spawn(
        &self,
        spec: &ActorSpec,
    ) -> Result<Arc<ActorVTable>, RuntimeError> {
        let cm = self.code_manager.lock().unwrap();
        cm.get_current_vtable(&spec.module_name)
            .ok_or(RuntimeError::RuntimeStopped)
    }

    /// Get the current version of a module.
    pub fn current_version(&self, module_name: &str) -> Option<String> {
        let cm = self.code_manager.lock().unwrap();
        cm.current_version(module_name)
    }

    /// List all loaded versions for a module.
    pub fn list_versions(&self, module_name: &str) -> Vec<String> {
        let cm = self.code_manager.lock().unwrap();
        cm.list_versions(module_name)
    }

    /// Get the runtime sender for spawning.
    pub fn runtime_sender(&self) -> &RuntimeSender {
        &self.runtime_sender
    }
}
