use runact::{
    Actor, ActorContext, ActorError, Capability, ResourceHandle, ResourceRegistry, Runtime,
};
use std::any::Any;
use std::time::Duration;

#[derive(Debug)]
struct FileHandle {
    path: String,
    writable: bool,
}

impl ResourceHandle for FileHandle {
    fn resource_type(&self) -> &str {
        "file"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct FileReaderActor {
    file_cap: Option<Capability<FileHandle>>,
}

#[derive(Clone)]
enum FileReaderMessage {
    SetCapability(Capability<FileHandle>),
    ReadFile,
}

impl Actor for FileReaderActor {
    type Message = FileReaderMessage;

    fn handle(&mut self, msg: FileReaderMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            FileReaderMessage::SetCapability(cap) => {
                self.file_cap = Some(cap);
            }
            FileReaderMessage::ReadFile => {
                if let Some(cap) = &self.file_cap {
                    let path = cap.handle().path.clone();
                    let _ = ctx.reply(path);
                } else {
                    let _ = ctx.reply("no capability".to_string());
                }
            }
        }
        Ok(())
    }
}

#[test]
fn test_capability_creation() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let actor = runtime
        .spawn(FileReaderActor { file_cap: None })
        .expect("Failed to spawn");

    let file = FileHandle {
        path: "/tmp/test.txt".to_string(),
        writable: false,
    };
    let cap = Capability::new(actor, file);

    assert_eq!(cap.owner(), actor);
    assert_eq!(cap.handle().path, "/tmp/test.txt");
    assert!(!cap.handle().writable);
}

#[test]
fn test_capability_clone() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let actor = runtime
        .spawn(FileReaderActor { file_cap: None })
        .expect("Failed to spawn");

    let file = FileHandle {
        path: "/tmp/test.txt".to_string(),
        writable: true,
    };
    let cap = Capability::new(actor, file);
    let cap2 = cap.clone();

    assert_eq!(cap.owner(), cap2.owner());
    assert_eq!(cap.handle().path, cap2.handle().path);
}

#[test]
fn test_actor_receives_capability() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let actor = runtime
        .spawn(FileReaderActor { file_cap: None })
        .expect("Failed to spawn");

    let file = FileHandle {
        path: "/tmp/test.txt".to_string(),
        writable: false,
    };
    let cap = Capability::new(actor, file);

    runtime
        .send(actor, FileReaderMessage::SetCapability(cap))
        .expect("Failed to send");
    std::thread::sleep(Duration::from_millis(50));

    let handle = runtime
        .request(actor, FileReaderMessage::ReadFile)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let path = reply.downcast::<String>().expect("Failed to downcast");
    assert_eq!(*path, "/tmp/test.txt");
}

#[test]
fn test_resource_registry() {
    let registry = ResourceRegistry::new();
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let actor = runtime
        .spawn(FileReaderActor { file_cap: None })
        .expect("Failed to spawn");

    let file = FileHandle {
        path: "/tmp/test.txt".to_string(),
        writable: true,
    };
    let cap = Capability::new(actor, file);
    registry.register(cap);

    let retrieved: Option<Capability<FileHandle>> = registry.get();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().handle().path, "/tmp/test.txt");
}

#[test]
fn test_resource_registry_remove() {
    let registry = ResourceRegistry::new();
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let actor = runtime
        .spawn(FileReaderActor { file_cap: None })
        .expect("Failed to spawn");

    let file = FileHandle {
        path: "/tmp/test.txt".to_string(),
        writable: false,
    };
    let cap = Capability::new(actor, file);
    registry.register(cap);

    assert!(registry.has::<FileHandle>());
    let removed: Option<Capability<FileHandle>> = registry.remove();
    assert!(removed.is_some());
    assert!(!registry.has::<FileHandle>());
}
