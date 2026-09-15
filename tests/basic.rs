use std::sync::{Arc, Mutex};
use std::time::Duration;
use runact::{Actor, ActorContext, ActorError, Runtime, RuntimeConfig, RuntimeError};

/// Simple counter actor for testing.
struct CounterActor {
    count: u64,
}

enum CounterMessage {
    Increment,
    _Decrement,
    GetValue,
}

impl Actor for CounterActor {
    type Message = CounterMessage;

    fn handle(&mut self, msg: CounterMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            CounterMessage::Increment => {
                self.count += 1;
            }
            CounterMessage::_Decrement => {
                self.count = self.count.saturating_sub(1);
            }
            CounterMessage::GetValue => {
                let _ = ctx.reply(self.count);
            }
        }
        Ok(())
    }
}

/// Actor that tracks received messages.
struct LoggerActor {
    messages: Arc<Mutex<Vec<String>>>,
}

enum LoggerMessage {
    Log(String),
    GetCount,
}

impl Actor for LoggerActor {
    type Message = LoggerMessage;

    fn handle(&mut self, msg: LoggerMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            LoggerMessage::Log(text) => {
                self.messages.lock().unwrap().push(text);
            }
            LoggerMessage::GetCount => {
                let count = self.messages.lock().unwrap().len();
                let _ = ctx.reply(count);
            }
        }
        Ok(())
    }
}

/// Actor that sends messages to another actor.
struct ForwardActor {
    target: Option<runact::ActorId>,
}

enum ForwardMessage {
    SetTarget(runact::ActorId),
    Forward(String),
}

impl Actor for ForwardActor {
    type Message = ForwardMessage;

    fn handle(&mut self, msg: ForwardMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            ForwardMessage::SetTarget(id) => {
                self.target = Some(id);
            }
            ForwardMessage::Forward(text) => {
                if let Some(target) = self.target {
                    let _ = ctx.send_to(target, LoggerMessage::Log(text));
                }
            }
        }
        Ok(())
    }
}

#[test]
fn test_runtime_creation() {
    let runtime = Runtime::new();
    assert!(runtime.is_ok());
}

#[test]
fn test_actor_spawning() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let actor_id = runtime.spawn(CounterActor { count: 0 });
    assert!(actor_id.is_ok());
}

#[test]
fn test_multiple_actors() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    
    let actor1 = runtime.spawn(CounterActor { count: 0 }).expect("Failed to spawn actor1");
    let actor2 = runtime.spawn(CounterActor { count: 10 }).expect("Failed to spawn actor2");
    let actor3 = runtime.spawn(CounterActor { count: 100 }).expect("Failed to spawn actor3");
    
    assert_ne!(actor1, actor2);
    assert_ne!(actor2, actor3);
    assert_ne!(actor1, actor3);
}

#[test]
fn test_actor_listing() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    
    let _ = runtime.spawn(CounterActor { count: 0 }).expect("Failed to spawn actor");
    let _ = runtime.spawn(CounterActor { count: 10 }).expect("Failed to spawn actor");
    
    let actors = runtime.list_actors().expect("Failed to list actors");
    assert_eq!(actors.len(), 2);
}

#[test]
fn test_actor_inspection() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    
    let actor_id = runtime.spawn(CounterActor { count: 42 }).expect("Failed to spawn actor");
    let info = runtime.inspect_actor(actor_id).expect("Failed to inspect actor");
    
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.id, actor_id);
}

#[test]
fn test_runtime_shutdown() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    
    let _ = runtime.spawn(CounterActor { count: 0 }).expect("Failed to spawn actor");
    let _ = runtime.spawn(CounterActor { count: 10 }).expect("Failed to spawn actor");
    
    let result = runtime.shutdown();
    assert!(result.is_ok());
}

#[test]
fn test_actor_id_uniqueness() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    
    let mut ids = Vec::new();
    for _ in 0..100 {
        let id = runtime.spawn(CounterActor { count: 0 }).expect("Failed to spawn actor");
        ids.push(id);
    }
    
    // Check all IDs are unique
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique_ids.len(), 100);
}

// ============================================================
// Real message passing tests
// ============================================================

#[test]
fn test_send_fire_and_forget() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let counter = runtime.spawn(CounterActor { count: 0 }).expect("Failed to spawn");

    // Send 5 increments
    for _ in 0..5 {
        runtime.send(counter, CounterMessage::Increment).expect("Failed to send");
    }

    // Verify by requesting the value
    let handle = runtime.request(counter, CounterMessage::GetValue).expect("Failed to request");
    let reply = handle.recv_timeout(Duration::from_secs(1)).expect("Failed to receive reply");
    let count = reply.downcast::<u64>().expect("Failed to downcast");
    assert_eq!(*count, 5);
}

#[test]
fn test_request_reply() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let counter = runtime.spawn(CounterActor { count: 42 }).expect("Failed to spawn");

    // Request current value
    let handle = runtime.request(counter, CounterMessage::GetValue).expect("Failed to request");
    let reply = handle.recv_timeout(Duration::from_secs(1)).expect("Failed to receive reply");
    let count = reply.downcast::<u64>().expect("Failed to downcast");
    assert_eq!(*count, 42);
}

#[test]
fn test_request_blocking() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let counter = runtime.spawn(CounterActor { count: 99 }).expect("Failed to spawn");

    // Blocking request with timeout
    let count: u64 = runtime.request_blocking(
        counter,
        CounterMessage::GetValue,
        Some(Duration::from_secs(1)),
    ).expect("Failed to receive reply");
    assert_eq!(count, 99);
}

#[test]
fn test_actor_to_actor_communication() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");

    let messages: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let logger = runtime.spawn(LoggerActor {
        messages: messages.clone(),
    }).expect("Failed to spawn logger");

    let forwarder = runtime.spawn(ForwardActor { target: None }).expect("Failed to spawn forwarder");

    // Set the forwarder's target to the logger
    runtime.send(forwarder, ForwardMessage::SetTarget(logger)).expect("Failed to send");

    // Send through the forwarder
    runtime.send(forwarder, ForwardMessage::Forward("hello".to_string())).expect("Failed to send");
    runtime.send(forwarder, ForwardMessage::Forward("world".to_string())).expect("Failed to send");

    std::thread::sleep(Duration::from_millis(50));

    // Verify logger received both messages
    let handle = runtime.request(logger, LoggerMessage::GetCount).expect("Failed to request");
    let reply = handle.recv_timeout(Duration::from_secs(1)).expect("Failed to receive reply");
    let count = reply.downcast::<usize>().expect("Failed to downcast");
    assert_eq!(*count, 2);

    let msgs = messages.lock().unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0], "hello");
    assert_eq!(msgs[1], "world");
}

#[test]
fn test_request_to_nonexistent_actor() {
    let runtime = Runtime::new().expect("Failed to create runtime");
    let fake_id = runact::ActorId::new(999);

    let result = runtime.send(fake_id, "hello".to_string());
    assert!(result.is_err());
}

#[test]
fn test_backpressure_rejects_when_full() {
    struct SlowActor;

    impl Actor for SlowActor {
        type Message = u64;

        fn handle(&mut self, _msg: u64, _ctx: &mut ActorContext) -> Result<(), ActorError> {
            std::thread::sleep(Duration::from_millis(50));
            Ok(())
        }
    }

    let config = RuntimeConfig {
        mailbox_capacity: 2,
        ..RuntimeConfig::default()
    };
    let mut runtime = Runtime::with_config(config).expect("create runtime");
    let id = runtime.spawn(SlowActor).expect("spawn");

    std::thread::sleep(Duration::from_millis(30));

    let mut got_full = false;
    for _ in 0..20 {
        if let Err(RuntimeError::MailboxFull(_)) = runtime.send(id, 1u64) {
            got_full = true;
            break;
        }
    }
    assert!(got_full, "expected MailboxFull error");
}

#[test]
fn test_send_blocking_does_not_reject() {
    let config = RuntimeConfig {
        mailbox_capacity: 1,
        ..RuntimeConfig::default()
    };
    let mut runtime = Runtime::with_config(config).expect("create runtime");
    let id = runtime.spawn(CounterActor { count: 0 }).expect("spawn");

    for _ in 0..10 {
        let result = runtime.send_blocking(id, CounterMessage::Increment);
        assert!(result.is_ok());
    }
}

#[test]
fn test_multiple_requests_concurrent() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let counter = runtime.spawn(CounterActor { count: 0 }).expect("Failed to spawn");

    // Send 10 increments
    for _ in 0..10 {
        runtime.send(counter, CounterMessage::Increment).expect("Failed to send");
    }

    // Make 5 concurrent requests
    let mut handles = Vec::new();
    for _ in 0..5 {
        let handle = runtime.request(counter, CounterMessage::GetValue).expect("Failed to request");
        handles.push(handle);
    }

    // All should return the same value (10)
    for handle in handles {
        let reply = handle.recv_timeout(Duration::from_secs(1)).expect("Failed to receive reply");
        let count = reply.downcast::<u64>().expect("Failed to downcast");
        assert_eq!(*count, 10);
    }
}
