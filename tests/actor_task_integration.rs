use runact::{Actor, ActorContext, ActorError, Runtime};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct ComputeActor {
    result: Arc<AtomicU64>,
}

impl Actor for ComputeActor {
    type Message = u64;

    fn handle(&mut self, msg: u64, _ctx: &mut ActorContext) -> Result<(), ActorError> {
        self.result.store(msg, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn test_actor_spawns_task() {
    let mut runtime = Runtime::new().expect("create runtime");
    let result = Arc::new(AtomicU64::new(0));
    let result_clone = result.clone();

    let actor = ComputeActor {
        result: result_clone,
    };
    let _id = runtime.spawn(actor).expect("spawn actor");

    let handle = runtime.spawn_task(async { 42u64 }).expect("spawn task");

    let task_result = handle
        .recv_timeout(Duration::from_secs(2))
        .expect("recv task");
    assert_eq!(task_result, 42);
}

#[test]
fn test_actor_receives_task_result() {
    let mut runtime = Runtime::new().expect("create runtime");
    let result = Arc::new(AtomicU64::new(0));
    let result_clone = result.clone();

    let actor = ComputeActor {
        result: result_clone,
    };
    let id = runtime.spawn(actor).expect("spawn actor");

    let handle = runtime.spawn_task(async { 42u64 }).expect("spawn task");

    let task_result = handle
        .recv_timeout(Duration::from_secs(2))
        .expect("recv task");
    runtime.send(id, task_result).expect("send to actor");

    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(result.load(Ordering::SeqCst), 42);
}

#[test]
fn test_actor_stays_responsive_during_task() {
    let mut runtime = Runtime::new().expect("create runtime");
    let result = Arc::new(AtomicU64::new(0));
    let result_clone = result.clone();

    let actor = ComputeActor {
        result: result_clone,
    };
    let id = runtime.spawn(actor).expect("spawn actor");

    let _handle = runtime
        .spawn_task(async {
            runact::Runtime::sleep(Duration::from_millis(100)).await;
            42u64
        })
        .expect("spawn task");

    runtime.send_blocking(id, 1u64).expect("send to actor");
    // Poll until the actor processes the message (actor loop uses recv_timeout).
    let deadline = Instant::now() + Duration::from_secs(2);
    while result.load(Ordering::SeqCst) != 1 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(result.load(Ordering::SeqCst), 1);

    runtime.send_blocking(id, 2u64).expect("send to actor");
    let deadline = Instant::now() + Duration::from_secs(2);
    while result.load(Ordering::SeqCst) != 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(result.load(Ordering::SeqCst), 2);
}

#[test]
fn test_multiple_tasks_to_actor() {
    let mut runtime = Runtime::new().expect("create runtime");
    let result = Arc::new(AtomicU64::new(0));
    let result_clone = result.clone();

    let actor = ComputeActor {
        result: result_clone,
    };
    let id = runtime.spawn(actor).expect("spawn actor");

    let mut handles = Vec::new();
    for i in 0..5u64 {
        handles.push(
            runtime
                .spawn_task(async move { i * 10 })
                .expect("spawn task"),
        );
    }

    for handle in handles {
        let task_result = handle
            .recv_timeout(Duration::from_secs(2))
            .expect("recv task");
        runtime.send(id, task_result).expect("send to actor");
    }

    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(result.load(Ordering::SeqCst), 40);
}

#[test]
fn test_task_panic_does_not_affect_actor() {
    let mut runtime = Runtime::new().expect("create runtime");
    let result = Arc::new(AtomicU64::new(0));
    let result_clone = result.clone();

    let actor = ComputeActor {
        result: result_clone,
    };
    let id = runtime.spawn(actor).expect("spawn actor");

    let _panicker = runtime
        .spawn_task(async {
            panic!("task panicked");
        })
        .expect("spawn task");

    std::thread::sleep(Duration::from_millis(50));

    let handle = runtime.spawn_task(async { 99u64 }).expect("spawn task");

    let task_result = handle
        .recv_timeout(Duration::from_secs(2))
        .expect("recv task");
    runtime.send(id, task_result).expect("send to actor");

    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(result.load(Ordering::SeqCst), 99);
}
