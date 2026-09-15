use runact::Scheduler;

#[test]
fn test_scheduler_creation() {
    let scheduler = Scheduler::new();
    assert!(scheduler.worker_count() > 0);
}

#[test]
fn test_scheduler_spawning() {
    let scheduler = Scheduler::new();
    
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counter_clone = counter.clone();
    
    scheduler.spawn(move || {
        counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    
    // Give worker time to execute
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn test_scheduler_multiple_tasks() {
    let scheduler = Scheduler::new();
    
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    
    for _ in 0..100 {
        let counter_clone = counter.clone();
        scheduler.spawn(move || {
            counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
    }
    
    // Give workers time to execute
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 100);
}
