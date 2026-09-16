use runact::{Actor, ActorContext, ActorError, ComputeHandle, Runtime};
use std::time::Duration;

struct ComputeActor {
    pending: Option<ComputeHandle<u64>>,
}

enum ComputeMessage {
    SubmitTask(u64),
    CheckResult,
    GetResult,
}

impl Actor for ComputeActor {
    type Message = ComputeMessage;

    fn handle(&mut self, msg: ComputeMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            ComputeMessage::SubmitTask(input) => {
                let handle = ctx
                    .spawn_compute(move || input * 2)
                    .expect("Failed to submit compute task");
                self.pending = Some(handle);
            }
            ComputeMessage::CheckResult => {
                if let Some(ref handle) = self.pending {
                    if let Some(result) = handle.try_recv() {
                        match result {
                            Ok(value) => {
                                let _ = ctx.reply(Some(value));
                                self.pending = None;
                            }
                            Err(_) => {
                                let _ = ctx.reply(None::<u64>);
                            }
                        }
                    } else {
                        let _ = ctx.reply(None::<u64>);
                    }
                } else {
                    let _ = ctx.reply(None::<u64>);
                }
            }
            ComputeMessage::GetResult => {
                if let Some(ref handle) = self.pending {
                    match handle.recv_timeout(Duration::from_secs(1)) {
                        Ok(value) => {
                            let _ = ctx.reply(Some(value));
                            self.pending = None;
                        }
                        Err(_) => {
                            let _ = ctx.reply(None::<u64>);
                        }
                    }
                } else {
                    let _ = ctx.reply(None::<u64>);
                }
            }
        }
        Ok(())
    }
}

struct PanicComputeActor;

enum PanicComputeMessage {
    SubmitPanicTask,
    IsAlive,
}

impl Actor for PanicComputeActor {
    type Message = PanicComputeMessage;

    fn handle(
        &mut self,
        msg: PanicComputeMessage,
        ctx: &mut ActorContext,
    ) -> Result<(), ActorError> {
        match msg {
            PanicComputeMessage::SubmitPanicTask => {
                let _ = ctx.spawn_compute(|| -> String {
                    panic!("Compute task panicked!");
                });
            }
            PanicComputeMessage::IsAlive => {
                let _ = ctx.reply(true);
            }
        }
        Ok(())
    }
}

#[test]
fn test_actor_submits_compute_work() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let actor = runtime
        .spawn(ComputeActor { pending: None })
        .expect("Failed to spawn");

    runtime
        .send(actor, ComputeMessage::SubmitTask(21))
        .expect("Failed to send");
    std::thread::sleep(Duration::from_millis(50));

    let handle = runtime
        .request(actor, ComputeMessage::GetResult)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let result = reply.downcast::<Option<u64>>().expect("Failed to downcast");
    assert_eq!(*result, Some(42));
}

#[test]
fn test_compute_panic_isolation() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let actor = runtime.spawn(PanicComputeActor).expect("Failed to spawn");

    runtime
        .send(actor, PanicComputeMessage::SubmitPanicTask)
        .expect("Failed to send");
    std::thread::sleep(Duration::from_millis(50));

    let handle = runtime
        .request(actor, PanicComputeMessage::IsAlive)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let alive = reply.downcast::<bool>().expect("Failed to downcast");
    assert!(*alive);
}

#[test]
fn test_compute_handle_try_recv() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let actor = runtime
        .spawn(ComputeActor { pending: None })
        .expect("Failed to spawn");

    runtime
        .send(actor, ComputeMessage::SubmitTask(10))
        .expect("Failed to send");
    std::thread::sleep(Duration::from_millis(50));

    let handle = runtime
        .request(actor, ComputeMessage::CheckResult)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let result = reply.downcast::<Option<u64>>().expect("Failed to downcast");
    assert_eq!(*result, Some(20));
}

#[test]
fn test_external_compute_submission() {
    let runtime = Runtime::new().expect("Failed to create runtime");

    let handle = runtime
        .compute()
        .spawn(|| 100 + 200)
        .expect("Failed to submit compute task");

    let result = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    assert_eq!(result, 300);
}

#[test]
fn test_multiple_compute_tasks() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let actor = runtime
        .spawn(ComputeActor { pending: None })
        .expect("Failed to spawn");

    runtime
        .send(actor, ComputeMessage::SubmitTask(5))
        .expect("Failed to send");
    std::thread::sleep(Duration::from_millis(50));

    let handle = runtime
        .request(actor, ComputeMessage::GetResult)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let result = reply.downcast::<Option<u64>>().expect("Failed to downcast");
    assert_eq!(*result, Some(10));

    runtime
        .send(actor, ComputeMessage::SubmitTask(25))
        .expect("Failed to send");
    std::thread::sleep(Duration::from_millis(50));

    let handle = runtime
        .request(actor, ComputeMessage::GetResult)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let result = reply.downcast::<Option<u64>>().expect("Failed to downcast");
    assert_eq!(*result, Some(50));
}

#[test]
fn test_compute_cancellation() {
    let runtime = Runtime::new().expect("create runtime");

    let handle = runtime
        .compute()
        .spawn(|| {
            std::thread::sleep(Duration::from_secs(10));
            42u64
        })
        .expect("submit task");

    handle.cancel();
    assert!(handle.is_cancelled());

    let result = handle.recv_timeout(Duration::from_millis(500));
    assert!(result.is_err());
}

#[test]
fn test_compute_cancel_before_execution() {
    let runtime = Runtime::new().expect("create runtime");

    let (ready_tx, ready_rx) = crossbeam_channel::bounded::<()>(1);
    let (proceed_tx, proceed_rx) = crossbeam_channel::bounded::<()>(1);

    let handle = runtime
        .compute()
        .spawn(move || {
            ready_tx.send(()).ok();
            proceed_rx.recv().ok();
            42u64
        })
        .expect("submit task");

    ready_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("task should start");

    handle.cancel();
    proceed_tx.send(()).ok();

    let result = handle.recv_timeout(Duration::from_secs(2));
    assert!(result.is_err());
}
