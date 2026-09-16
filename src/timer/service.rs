use crate::actor::ActorId;
use crate::runtime::MessageEnvelope;
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

type MessageFactory = Box<dyn Fn() -> Box<dyn std::any::Any + Send> + Send + Sync>;

pub struct TimerService {
    timers: Arc<Mutex<Vec<TimerEntry>>>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

struct TimerEntry {
    id: TimerId,
    actor_id: ActorId,
    scheduled_at: Instant,
    factory: MessageFactory,
    interval: Option<Duration>,
}

/// Unique identifier for a scheduled timer.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);

impl TimerId {
    /// Create a new unique `TimerId`.
    pub fn new() -> Self {
        use std::sync::atomic::AtomicU64;
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for TimerId {
    fn default() -> Self {
        Self::new()
    }
}

/// Clonable handle for scheduling timers from actor context.
#[must_use]
#[derive(Clone)]
pub struct TimerHandle {
    timers: Arc<Mutex<Vec<TimerEntry>>>,
}

impl TimerHandle {
    /// Schedule a one-shot timer. Returns a [`TimerId`] for cancellation.
    pub fn schedule_timer<M: Send + 'static>(
        &self,
        duration: Duration,
        actor_id: ActorId,
        message: M,
    ) -> TimerId {
        let id = TimerId::new();
        let message = Arc::new(Mutex::new(Some(
            Box::new(message) as Box<dyn std::any::Any + Send>
        )));
        let mut timers = self.timers.lock().unwrap();
        timers.push(TimerEntry {
            id,
            actor_id,
            scheduled_at: Instant::now() + duration,
            factory: Box::new(move || message.lock().unwrap().take().expect("timer fired twice")),
            interval: None,
        });
        id
    }

    /// Schedule a periodic timer. Returns a [`TimerId`] for cancellation.
    pub fn schedule_interval<M: Clone + Send + Sync + 'static>(
        &self,
        interval: Duration,
        actor_id: ActorId,
        message: M,
    ) -> TimerId {
        let id = TimerId::new();
        let message = Arc::new(message);
        let mut timers = self.timers.lock().unwrap();
        timers.push(TimerEntry {
            id,
            actor_id,
            scheduled_at: Instant::now() + interval,
            factory: Box::new(move || {
                Box::new((*message).clone()) as Box<dyn std::any::Any + Send>
            }),
            interval: Some(interval),
        });
        id
    }

    /// Cancel a scheduled timer by its ID.
    pub fn cancel_timer(&self, timer_id: TimerId) {
        let mut timers = self.timers.lock().unwrap();
        timers.retain(|t| t.id != timer_id);
    }
}

impl TimerService {
    pub(crate) fn new(
        _senders: Arc<
            std::sync::RwLock<HashMap<ActorId, crossbeam_channel::Sender<MessageEnvelope>>>,
        >,
    ) -> Self {
        let timers = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));

        let timers_clone = timers.clone();
        let senders_clone = _senders.clone();
        let stop_clone = stop.clone();

        let handle = std::thread::spawn(move || {
            Self::run(timers_clone, senders_clone, stop_clone);
        });

        Self {
            timers,
            stop,
            handle: Some(handle),
        }
    }

    fn run(
        timers: Arc<Mutex<Vec<TimerEntry>>>,
        senders: Arc<
            std::sync::RwLock<HashMap<ActorId, crossbeam_channel::Sender<MessageEnvelope>>>,
        >,
        stop: Arc<AtomicBool>,
    ) {
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            let now = Instant::now();
            let mut due_timers: Vec<(TimerId, ActorId, Instant, MessageFactory, Option<Duration>)> =
                Vec::new();

            {
                let mut timers = timers.lock().unwrap();
                let mut i = 0;
                while i < timers.len() {
                    if now >= timers[i].scheduled_at {
                        let timer = timers.remove(i);
                        due_timers.push((
                            timer.id,
                            timer.actor_id,
                            timer.scheduled_at,
                            timer.factory,
                            timer.interval,
                        ));
                    } else {
                        i += 1;
                    }
                }
            }

            for (id, actor_id, entry_scheduled_at, factory, interval) in due_timers {
                let message = factory();
                let senders = senders.read().unwrap();
                if let Some(sender) = senders.get(&actor_id) {
                    let _ = sender.send(MessageEnvelope::Message(message));
                }

                if let Some(interval) = interval {
                    let mut timers = timers.lock().unwrap();
                    timers.push(TimerEntry {
                        id,
                        actor_id,
                        scheduled_at: entry_scheduled_at + interval,
                        factory,
                        interval: Some(interval),
                    });
                }
            }

            std::thread::sleep(Duration::from_millis(1));
        }
    }

    pub fn handle(&self) -> TimerHandle {
        TimerHandle {
            timers: self.timers.clone(),
        }
    }

    pub fn schedule(
        &self,
        duration: Duration,
        actor_id: ActorId,
        message: Box<dyn std::any::Any + Send>,
    ) -> TimerId {
        let id = TimerId::new();
        let message = Arc::new(Mutex::new(Some(message)));
        let mut timers = self.timers.lock().unwrap();
        timers.push(TimerEntry {
            id,
            actor_id,
            scheduled_at: Instant::now() + duration,
            factory: Box::new(move || message.lock().unwrap().take().expect("timer fired twice")),
            interval: None,
        });
        id
    }

    pub fn schedule_interval(
        &self,
        interval: Duration,
        actor_id: ActorId,
        factory: MessageFactory,
    ) -> TimerId {
        let id = TimerId::new();
        let mut timers = self.timers.lock().unwrap();
        timers.push(TimerEntry {
            id,
            actor_id,
            scheduled_at: Instant::now() + interval,
            factory,
            interval: Some(interval),
        });
        id
    }

    pub fn cancel(&self, timer_id: TimerId) {
        let mut timers = self.timers.lock().unwrap();
        timers.retain(|t| t.id != timer_id);
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
