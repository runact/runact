use runact::{Actor, ActorContext, ActorError, Runtime, TimerId};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct TimerActor {
    received: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone)]
enum TimerMessage {
    Ping(String),
    GetCount,
}

impl Actor for TimerActor {
    type Message = TimerMessage;

    fn handle(&mut self, msg: TimerMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            TimerMessage::Ping(text) => {
                self.received.lock().unwrap().push(text);
            }
            TimerMessage::GetCount => {
                let count = self.received.lock().unwrap().len();
                let _ = ctx.reply(count);
            }
        }
        Ok(())
    }
}

#[test]
fn test_one_shot_timer() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let actor = runtime
        .spawn(TimerActor {
            received: received.clone(),
        })
        .expect("Failed to spawn");

    let _ = runtime.schedule_timer(
        Duration::from_millis(50),
        actor,
        TimerMessage::Ping("timer-fired".to_string()),
    );

    std::thread::sleep(Duration::from_millis(100));

    let handle = runtime
        .request(actor, TimerMessage::GetCount)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let count = reply.downcast::<usize>().expect("Failed to downcast");
    assert_eq!(*count, 1);

    let msgs = received.lock().unwrap();
    assert_eq!(msgs[0], "timer-fired");
}

#[test]
fn test_periodic_timer() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let actor = runtime
        .spawn(TimerActor {
            received: received.clone(),
        })
        .expect("Failed to spawn");

    let _ = runtime.schedule_interval(
        Duration::from_millis(30),
        actor,
        TimerMessage::Ping("tick".to_string()),
    );

    std::thread::sleep(Duration::from_millis(100));

    let handle = runtime
        .request(actor, TimerMessage::GetCount)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let count = reply.downcast::<usize>().expect("Failed to downcast");
    assert!(*count >= 2);
}

#[test]
fn test_timer_cancellation() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let actor = runtime
        .spawn(TimerActor {
            received: received.clone(),
        })
        .expect("Failed to spawn");

    let timer_id = runtime.schedule_timer(
        Duration::from_millis(50),
        actor,
        TimerMessage::Ping("should-not-fire".to_string()),
    );

    runtime.cancel_timer(timer_id);

    std::thread::sleep(Duration::from_millis(100));

    let handle = runtime
        .request(actor, TimerMessage::GetCount)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let count = reply.downcast::<usize>().expect("Failed to downcast");
    assert_eq!(*count, 0);
}

#[test]
fn test_multiple_timers() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let actor = runtime
        .spawn(TimerActor {
            received: received.clone(),
        })
        .expect("Failed to spawn");

    let _ = runtime.schedule_timer(
        Duration::from_millis(30),
        actor,
        TimerMessage::Ping("first".to_string()),
    );

    let _ = runtime.schedule_timer(
        Duration::from_millis(60),
        actor,
        TimerMessage::Ping("second".to_string()),
    );

    std::thread::sleep(Duration::from_millis(100));

    let handle = runtime
        .request(actor, TimerMessage::GetCount)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let count = reply.downcast::<usize>().expect("Failed to downcast");
    assert_eq!(*count, 2);

    let msgs = received.lock().unwrap();
    assert!(msgs.contains(&"first".to_string()));
    assert!(msgs.contains(&"second".to_string()));
}

struct SelfTimerActor {
    received: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone)]
enum SelfTimerMessage {
    Start,
    Ping(String),
    GetCount,
}

impl Actor for SelfTimerActor {
    type Message = SelfTimerMessage;

    fn handle(&mut self, msg: SelfTimerMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            SelfTimerMessage::Start => {
                let _ = ctx
                    .schedule_timer(
                        Duration::from_millis(30),
                        SelfTimerMessage::Ping("self-timer".to_string()),
                    )
                    .expect("Failed to schedule timer");
            }
            SelfTimerMessage::Ping(text) => {
                self.received.lock().unwrap().push(text);
            }
            SelfTimerMessage::GetCount => {
                let count = self.received.lock().unwrap().len();
                let _ = ctx.reply(count);
            }
        }
        Ok(())
    }
}

#[test]
fn test_actor_schedules_timer_from_handle() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let actor = runtime
        .spawn(SelfTimerActor {
            received: received.clone(),
        })
        .expect("Failed to spawn");

    runtime
        .send(actor, SelfTimerMessage::Start)
        .expect("Failed to send");

    std::thread::sleep(Duration::from_millis(100));

    let handle = runtime
        .request(actor, SelfTimerMessage::GetCount)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let count = reply.downcast::<usize>().expect("Failed to downcast");
    assert_eq!(*count, 1);

    let msgs = received.lock().unwrap();
    assert_eq!(msgs[0], "self-timer");
}

struct SelfIntervalActor {
    received: Arc<Mutex<Vec<String>>>,
    timer_id: Arc<Mutex<Option<TimerId>>>,
}

#[derive(Clone)]
enum SelfIntervalMessage {
    StartInterval,
    Ping(String),
    GetCount,
}

impl Actor for SelfIntervalActor {
    type Message = SelfIntervalMessage;

    fn handle(
        &mut self,
        msg: SelfIntervalMessage,
        ctx: &mut ActorContext,
    ) -> Result<(), ActorError> {
        match msg {
            SelfIntervalMessage::StartInterval => {
                let id = ctx
                    .schedule_interval(
                        Duration::from_millis(30),
                        SelfIntervalMessage::Ping("interval-tick".to_string()),
                    )
                    .expect("Failed to schedule interval");
                *self.timer_id.lock().unwrap() = Some(id);
            }
            SelfIntervalMessage::Ping(text) => {
                self.received.lock().unwrap().push(text);
            }
            SelfIntervalMessage::GetCount => {
                let count = self.received.lock().unwrap().len();
                let _ = ctx.reply(count);
            }
        }
        Ok(())
    }
}

#[test]
fn test_actor_schedules_interval_from_handle() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let timer_id = Arc::new(Mutex::new(None));
    let actor = runtime
        .spawn(SelfIntervalActor {
            received: received.clone(),
            timer_id: timer_id.clone(),
        })
        .expect("Failed to spawn");

    runtime
        .send(actor, SelfIntervalMessage::StartInterval)
        .expect("Failed to send");

    std::thread::sleep(Duration::from_millis(100));

    let handle = runtime
        .request(actor, SelfIntervalMessage::GetCount)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let count = reply.downcast::<usize>().expect("Failed to downcast");
    assert!(*count >= 2);

    if let Some(id) = timer_id.lock().unwrap().take() {
        runtime.cancel_timer(id);
    }
}

#[test]
fn test_actor_cancels_interval_from_handle() {
    let mut runtime = Runtime::new().expect("Failed to create runtime");
    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let timer_id = Arc::new(Mutex::new(None));
    let actor = runtime
        .spawn(SelfIntervalActor {
            received: received.clone(),
            timer_id: timer_id.clone(),
        })
        .expect("Failed to spawn");

    runtime
        .send(actor, SelfIntervalMessage::StartInterval)
        .expect("Failed to send");
    std::thread::sleep(Duration::from_millis(75));

    let id = timer_id.lock().unwrap().take().expect("Timer ID not set");
    runtime.cancel_timer(id);

    std::thread::sleep(Duration::from_millis(50));

    let handle = runtime
        .request(actor, SelfIntervalMessage::GetCount)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let count_before = *reply.downcast::<usize>().expect("Failed to downcast");

    std::thread::sleep(Duration::from_millis(200));

    let handle = runtime
        .request(actor, SelfIntervalMessage::GetCount)
        .expect("Failed to request");
    let reply = handle
        .recv_timeout(Duration::from_secs(1))
        .expect("Failed to receive");
    let count_after = *reply.downcast::<usize>().expect("Failed to downcast");

    assert_eq!(
        count_before, count_after,
        "Timer should have stopped after cancellation"
    );
}
