use runact::{CancellationToken, Runtime, TaskGroup};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

#[test]
fn test_cancellation_token_basic() {
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());

    token.cancel();
    assert!(token.is_cancelled());
}

#[test]
fn test_cancellation_token_child() {
    let parent = CancellationToken::new();
    let child = parent.child_token();

    assert!(!child.is_cancelled());

    parent.cancel();
    assert!(parent.is_cancelled());
    assert!(child.is_cancelled());
}

#[test]
fn test_cancellation_token_child_independent() {
    let parent = CancellationToken::new();
    let child = parent.child_token();

    child.cancel();
    assert!(child.is_cancelled());
    assert!(!parent.is_cancelled());
}

#[test]
fn test_cancellation_token_multiple_children() {
    let parent = CancellationToken::new();
    let child1 = parent.child_token();
    let child2 = parent.child_token();
    let child3 = parent.child_token();

    parent.cancel();
    assert!(child1.is_cancelled());
    assert!(child2.is_cancelled());
    assert!(child3.is_cancelled());
}

#[test]
fn test_cancellation_token_nested_children() {
    let root = CancellationToken::new();
    let child = root.child_token();
    let grandchild = child.child_token();

    root.cancel();
    assert!(child.is_cancelled());
    assert!(grandchild.is_cancelled());
}

#[test]
fn test_task_group_basic() {
    let runtime = Runtime::new().expect("create runtime");
    let group = TaskGroup::new();

    let handle = group.spawn(&runtime, async { 42u64 });
    let result = handle.recv_timeout(Duration::from_secs(2)).expect("recv");
    assert_eq!(result, 42);
}

#[test]
fn test_task_group_multiple_tasks() {
    let runtime = Runtime::new().expect("create runtime");
    let group = TaskGroup::new();

    let h1 = group.spawn(&runtime, async { 1u32 });
    let h2 = group.spawn(&runtime, async { 2u32 });
    let h3 = group.spawn(&runtime, async { 3u32 });

    assert_eq!(h1.recv_timeout(Duration::from_secs(2)).expect("recv"), 1);
    assert_eq!(h2.recv_timeout(Duration::from_secs(2)).expect("recv"), 2);
    assert_eq!(h3.recv_timeout(Duration::from_secs(2)).expect("recv"), 3);
}

#[test]
fn test_task_group_cancellation() {
    let runtime = Runtime::new().expect("create runtime");
    let group = TaskGroup::new();
    let token = group.cancellation_token();

    let started = Arc::new(AtomicBool::new(false));
    let started_clone = started.clone();

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let tx_clone = tx.clone();

    let handle = group.spawn(&runtime, async move {
        started_clone.store(true, Ordering::SeqCst);
        let _ = rx.recv();
    });

    std::thread::sleep(Duration::from_millis(50));
    assert!(started.load(Ordering::SeqCst));

    group.cancel();
    assert!(token.is_cancelled());

    drop(tx_clone);

    let result = handle.recv_timeout(Duration::from_secs(2));
    assert!(result.is_err());
}

#[test]
fn test_task_group_panic_isolation() {
    let runtime = Runtime::new().expect("create runtime");
    let group = TaskGroup::new();

    let _panicker = group.spawn(&runtime, async {
        panic!("task panicked");
    });

    let survivor = group.spawn(&runtime, async { 99u64 });

    assert_eq!(
        survivor.recv_timeout(Duration::from_secs(2)).expect("recv"),
        99
    );
}

#[test]
fn test_task_group_stress() {
    let runtime = Runtime::new().expect("create runtime");
    let group = TaskGroup::new();

    let count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for i in 0..1000u32 {
        let count = count.clone();
        handles.push(group.spawn(&runtime, async move {
            count.fetch_add(1, Ordering::SeqCst);
            i
        }));
    }

    for handle in handles {
        let _ = handle.recv_timeout(Duration::from_secs(5));
    }

    assert_eq!(count.load(Ordering::SeqCst), 1000);
}
