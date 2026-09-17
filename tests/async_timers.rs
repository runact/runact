use runact::Runtime;
use std::time::{Duration, Instant};

#[test]
fn test_sleep_basic() {
    let runtime = Runtime::new().expect("create runtime");
    let start = Instant::now();

    let handle = runtime
        .spawn_task(async {
            Runtime::sleep(Duration::from_millis(100)).await;
            42u64
        })
        .expect("spawn task");

    let result = handle.recv_timeout(Duration::from_secs(2)).expect("recv");
    let elapsed = start.elapsed();

    assert_eq!(result, 42);
    assert!(elapsed >= Duration::from_millis(100));
}

#[test]
fn test_sleep_multiple() {
    let runtime = Runtime::new().expect("create runtime");
    let start = Instant::now();

    let handle = runtime
        .spawn_task(async {
            Runtime::sleep(Duration::from_millis(50)).await;
            Runtime::sleep(Duration::from_millis(50)).await;
            99u64
        })
        .expect("spawn task");

    let result = handle.recv_timeout(Duration::from_secs(2)).expect("recv");
    let elapsed = start.elapsed();

    assert_eq!(result, 99);
    assert!(elapsed >= Duration::from_millis(100));
}

#[test]
fn test_timeout_completes() {
    let runtime = Runtime::new().expect("create runtime");

    let handle = runtime
        .spawn_task(async {
            Runtime::timeout(Duration::from_secs(2), async { 42u64 })
                .await
                .expect("timeout should not occur")
        })
        .expect("spawn task");

    let result = handle.recv_timeout(Duration::from_secs(3)).expect("recv");
    assert_eq!(result, 42);
}

#[test]
fn test_timeout_expires() {
    let runtime = Runtime::new().expect("create runtime");

    let handle = runtime
        .spawn_task(async {
            Runtime::timeout(Duration::from_millis(50), async {
                Runtime::sleep(Duration::from_secs(1)).await;
                42u64
            })
            .await
        })
        .expect("spawn task");

    let result = handle.recv_timeout(Duration::from_secs(2)).expect("recv");
    assert!(result.is_err());
}

#[test]
fn test_timeout_with_sleep() {
    let runtime = Runtime::new().expect("create runtime");
    let start = Instant::now();

    let handle = runtime
        .spawn_task(async {
            Runtime::timeout(Duration::from_secs(1), async {
                Runtime::sleep(Duration::from_millis(100)).await;
                42u64
            })
            .await
            .expect("timeout should not occur")
        })
        .expect("spawn task");

    let result = handle.recv_timeout(Duration::from_secs(2)).expect("recv");
    let elapsed = start.elapsed();

    assert_eq!(result, 42);
    assert!(elapsed >= Duration::from_millis(100));
    assert!(elapsed < Duration::from_secs(1));
}

#[test]
fn test_sleep_concurrent() {
    let runtime = Runtime::new().expect("create runtime");
    let start = Instant::now();

    let h1 = runtime
        .spawn_task(async {
            Runtime::sleep(Duration::from_millis(100)).await;
            1u32
        })
        .expect("spawn task");

    let h2 = runtime
        .spawn_task(async {
            Runtime::sleep(Duration::from_millis(100)).await;
            2u32
        })
        .expect("spawn task");

    let r1 = h1.recv_timeout(Duration::from_secs(2)).expect("recv");
    let r2 = h2.recv_timeout(Duration::from_secs(2)).expect("recv");
    let elapsed = start.elapsed();

    assert_eq!(r1, 1);
    assert_eq!(r2, 2);
    assert!(elapsed >= Duration::from_millis(100));
    assert!(elapsed < Duration::from_secs(1));
}
