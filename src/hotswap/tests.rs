/// Tests for the hot code loading module.
#[cfg(test)]
#[allow(clippy::module_inception)]
mod hotswap_tests {
    use crate::actor::{Actor, ActorContext, ActorError};
    use crate::error::RuntimeError;
    use crate::hotswap::CodeManager;
    use crate::hotswap::types::ActorVTable as VTable;

    /// Test CodeManager version tracking
    #[test]
    fn test_codemanager_empty_state() {
        let cm = CodeManager::new();
        assert!(!cm.has_module("MyActor"));
        assert!(cm.get_current_vtable("MyActor").is_none());
    }

    /// Test that CodeManager tracks versions correctly
    #[test]
    fn test_codemanager_no_versions() {
        let cm = CodeManager::new();
        assert!(cm.list_versions("NonExistent").is_empty());
        assert!(cm.current_version("NonExistent").is_none());
        assert_eq!(cm.version_count("NonExistent"), 0);
    }

    /// Test that CodeManager has no module initially
    #[test]
    fn test_codemanager_no_module() {
        let cm = CodeManager::new();
        assert!(!cm.has_module("UserActor"));
        assert!(!cm.has_module("PaymentActor"));
    }

    /// Test ActorVTable structure compiles
    #[test]
    fn test_actor_vtable_type_exists() {
        fn dummy_handle(
            _ptr: *mut (),
            _msg: Box<dyn std::any::Any + Send>,
            _ctx: &mut crate::actor::ActorContext,
        ) -> Result<(), crate::actor::ActorError> {
            Ok(())
        }

        fn dummy_serialize(_ptr: *mut ()) -> Vec<u8> {
            Vec::new()
        }

        fn dummy_deserialize(_state: Vec<u8>) -> Result<*mut (), RuntimeError> {
            Ok(std::ptr::null_mut())
        }

        fn dummy_migrate(_state: Vec<u8>, _version: &str) -> Result<Vec<u8>, RuntimeError> {
            Ok(Vec::new())
        }

        fn dummy_drop(_ptr: *mut ()) {}

        let vtable = VTable {
            version: "1.0.0".to_string(),
            type_name: "TestActor".to_string(),
            handle_message_fn: dummy_handle,
            serialize_fn: dummy_serialize,
            deserialize_fn: dummy_deserialize,
            migrate_fn: dummy_migrate,
            drop_fn: dummy_drop,
        };

        assert_eq!(vtable.version, "1.0.0");
        assert_eq!(vtable.type_name, "TestActor");
    }

    /// Test HotSwappable trait compiles and works
    #[test]
    fn test_hot_swappable_trait_compiles() {
        use crate::hotswap::HotSwappable;

        struct TestActor {
            count: u64,
        }

        impl Actor for TestActor {
            type Message = String;

            fn handle(&mut self, _msg: String, _ctx: &mut ActorContext) -> Result<(), ActorError> {
                Ok(())
            }
        }

        impl HotSwappable for TestActor {
            fn current_version(&self) -> String {
                "1.0.0".to_string()
            }

            fn serialize_state(&self) -> Vec<u8> {
                self.count.to_le_bytes().to_vec()
            }

            fn deserialize_state(state: Vec<u8>) -> Result<Self, RuntimeError>
            where
                Self: Sized,
            {
                if state.len() >= 8 {
                    let count = u64::from_le_bytes([
                        state[0], state[1], state[2], state[3], state[4], state[5], state[6],
                        state[7],
                    ]);
                    Ok(TestActor { count })
                } else {
                    Ok(TestActor { count: 0 })
                }
            }
        }

        let actor = TestActor { count: 42 };
        let serialized = actor.serialize_state();
        let deserialized = TestActor::deserialize_state(serialized).unwrap();
        assert_eq!(actor.count, deserialized.count);
    }

    /// Test WorkItem type
    #[test]
    fn test_work_item_types() {
        use crate::hotswap::types::WorkItem;

        let work: WorkItem<String> = WorkItem {
            id: 1,
            payload: "test".to_string(),
        };
        assert_eq!(work.id, 1);
        assert_eq!(work.payload, "test");
    }

    /// Test MigrationRegistry
    #[test]
    fn test_migration_registry() {
        use crate::hotswap::migration::MigrationRegistry;

        let registry = MigrationRegistry::new();
        let state = vec![1, 2, 3];

        // Identity migration (same version) should succeed
        let result = registry.migrate("MyActor", "1.0.0", "1.0.0", state.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), state);

        // Migration to different version without registered function should fail
        let result = registry.migrate("MyActor", "1.0.0", "2.0.0", state);
        assert!(result.is_err());
    }

    /// Test DynamicSupervisor creation and version lookup
    #[test]
    fn test_dynamic_supervisor_creation() {
        use crate::hotswap::{ActorFactory, CodeManager, DynamicSupervisor};
        use crate::runtime::RuntimeSender;
        use std::collections::HashMap;
        use std::sync::atomic::AtomicU64;
        use std::sync::{Arc, Mutex, RwLock};

        // Define a test payload type that implements WorkPayload
        struct TestPayload(Vec<u8>);
        impl crate::hotswap::dynamic_supervisor::WorkPayload for TestPayload {
            fn to_bytes(&self) -> Vec<u8> {
                self.0.clone()
            }
        }

        let cm = Arc::new(Mutex::new(CodeManager::new()));
        let factory = Arc::new(ActorFactory::new(
            cm,
            RuntimeSender {
                senders: Arc::new(RwLock::new(HashMap::new())),
                request_counter: Arc::new(AtomicU64::new(0)),
            },
        ));

        let supervisor: DynamicSupervisor<TestPayload, String> =
            DynamicSupervisor::new("TestActor", factory);

        // No version loaded yet, should return None
        assert!(supervisor.current_version().is_none());
    }

    /// Test ActorFactory::prepare_dynamic_spawn returns current version
    #[test]
    fn test_factory_prepare_dynamic_spawn() {
        use crate::hotswap::{ActorFactory, CodeManager};
        use crate::runtime::RuntimeSender;
        use std::collections::HashMap;
        use std::sync::atomic::AtomicU64;
        use std::sync::{Arc, Mutex, RwLock};

        let cm = Arc::new(Mutex::new(CodeManager::new()));
        let factory = ActorFactory::new(
            cm.clone(),
            RuntimeSender {
                senders: Arc::new(RwLock::new(HashMap::new())),
                request_counter: Arc::new(AtomicU64::new(0)),
            },
        );

        // No module loaded → spawn should fail
        let spec = crate::hotswap::types::ActorSpec {
            module_name: "TestModule".to_string(),
            initial_state: vec![],
        };
        let result = factory.prepare_dynamic_spawn(&spec);
        assert!(result.is_err());

        // current_version should be None for unloaded module
        assert!(factory.current_version("TestModule").is_none());
    }
}
