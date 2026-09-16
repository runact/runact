use crate::actor::{Actor, ActorContext, ActorError, ActorId};
use crate::supervision::strategy::RestartStrategy;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[allow(dead_code)]
pub enum SupervisorMessage {
    ChildStarted { child_id: ActorId, name: String },
    ChildCrashed { child_id: ActorId, reason: String },
    GetStatus,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ChildStatus {
    pub id: ActorId,
    pub name: String,
    pub restart_count: usize,
    pub alive: bool,
}

#[allow(dead_code)]
pub struct SupervisorActor {
    children: HashMap<ActorId, ChildEntry>,
    strategy: RestartStrategy,
}

#[allow(dead_code)]
struct ChildEntry {
    name: String,
    restart_count: usize,
    alive: bool,
    last_restart_at: Option<Instant>,
}

#[allow(dead_code)]
impl SupervisorActor {
    pub fn new(strategy: RestartStrategy) -> Self {
        Self {
            children: HashMap::new(),
            strategy,
        }
    }

    fn max_restarts(&self) -> usize {
        match &self.strategy {
            RestartStrategy::OneForOne { max_restarts, .. } => *max_restarts,
        }
    }

    fn base_backoff(&self) -> Duration {
        match &self.strategy {
            RestartStrategy::OneForOne { base_backoff, .. } => *base_backoff,
        }
    }

    fn backoff_delay(&self, restart_count: usize) -> Duration {
        let base = self.base_backoff();
        let shift = (restart_count as u32).saturating_sub(1).min(30);
        let multiplier = 1u32 << shift;
        let delay = base * multiplier;
        delay.min(Duration::from_secs(30))
    }
}

impl Actor for SupervisorActor {
    type Message = SupervisorMessage;

    fn handle(&mut self, msg: SupervisorMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            SupervisorMessage::ChildStarted { child_id, name } => {
                self.children.insert(
                    child_id,
                    ChildEntry {
                        name,
                        restart_count: 0,
                        alive: true,
                        last_restart_at: None,
                    },
                );
            }
            SupervisorMessage::ChildCrashed { child_id, reason } => {
                let max_restarts = self.max_restarts();
                let new_restart_count = self
                    .children
                    .get(&child_id)
                    .map(|e| e.restart_count + 1)
                    .unwrap_or(0);
                let delay = self.backoff_delay(new_restart_count);

                if let Some(entry) = self.children.get_mut(&child_id) {
                    entry.restart_count = new_restart_count;
                    let can_restart = new_restart_count <= max_restarts;

                    if can_restart {
                        if let Some(last) = entry.last_restart_at {
                            let elapsed = last.elapsed();
                            if elapsed < delay {
                                tracing::warn!(
                                    child_id = %child_id,
                                    name = %entry.name,
                                    restart_count = new_restart_count,
                                    backoff_remaining_ms = (delay - elapsed).as_millis() as u64,
                                    reason = %reason,
                                    "child crashed, backoff not elapsed, staying dead"
                                );
                                entry.alive = false;
                                return Ok(());
                            }
                        }

                        entry.alive = true;
                        entry.last_restart_at = Some(Instant::now());
                        tracing::warn!(
                            child_id = %child_id,
                            name = %entry.name,
                            restart_count = new_restart_count,
                            max_restarts = max_restarts,
                            backoff_ms = delay.as_millis() as u64,
                            reason = %reason,
                            "child crashed, restarting"
                        );
                    } else {
                        entry.alive = false;
                        tracing::error!(
                            child_id = %child_id,
                            name = %entry.name,
                            restart_count = new_restart_count,
                            max_restarts = max_restarts,
                            reason = %reason,
                            "child exceeded max restarts, giving up"
                        );
                    }
                }
            }
            SupervisorMessage::GetStatus => {
                let statuses: Vec<ChildStatus> = self
                    .children
                    .iter()
                    .map(|(id, entry)| ChildStatus {
                        id: *id,
                        name: entry.name.clone(),
                        restart_count: entry.restart_count,
                        alive: entry.alive,
                    })
                    .collect();
                let _ = ctx.reply(statuses);
            }
        }
        Ok(())
    }
}

#[allow(dead_code)]
pub struct TestCounter {
    pub count: u64,
}

#[allow(dead_code)]
pub enum TestCounterMessage {
    Increment,
    GetValue,
}

impl Actor for TestCounter {
    type Message = TestCounterMessage;

    fn handle(
        &mut self,
        msg: TestCounterMessage,
        ctx: &mut ActorContext,
    ) -> Result<(), ActorError> {
        match msg {
            TestCounterMessage::Increment => {
                self.count += 1;
            }
            TestCounterMessage::GetValue => {
                let _ = ctx.reply(self.count);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Runtime;
    use crate::supervision::strategy::RestartStrategy;
    use std::time::Duration;

    #[test]
    fn test_supervisor_tracks_children() {
        let mut runtime = Runtime::new().expect("Failed to create runtime");
        let supervisor = SupervisorActor::new(RestartStrategy::default());
        let supervisor_id = runtime
            .spawn(supervisor)
            .expect("Failed to spawn supervisor");
        let child = runtime
            .spawn(TestCounter { count: 0 })
            .expect("Failed to spawn child");

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildStarted {
                    child_id: child,
                    name: "test_counter".to_string(),
                },
            )
            .expect("Failed to send");

        std::thread::sleep(Duration::from_millis(20));

        let handle = runtime
            .request(supervisor_id, SupervisorMessage::GetStatus)
            .expect("Failed to request");
        let reply = handle
            .recv_timeout(Duration::from_secs(1))
            .expect("Failed to receive");
        let statuses = reply
            .downcast::<Vec<ChildStatus>>()
            .expect("Failed to downcast");

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].name, "test_counter");
        assert_eq!(statuses[0].restart_count, 0);
        assert!(statuses[0].alive);
    }

    #[test]
    fn test_supervisor_handles_crash() {
        let mut runtime = Runtime::new().expect("Failed to create runtime");
        let strategy = RestartStrategy::OneForOne {
            max_restarts: 3,
            within: Duration::from_secs(60),
            base_backoff: Duration::from_millis(0),
        };
        let supervisor = SupervisorActor::new(strategy);
        let supervisor_id = runtime
            .spawn(supervisor)
            .expect("Failed to spawn supervisor");
        let child = runtime
            .spawn(TestCounter { count: 0 })
            .expect("Failed to spawn child");

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildStarted {
                    child_id: child,
                    name: "crasher".to_string(),
                },
            )
            .expect("Failed to send");

        std::thread::sleep(Duration::from_millis(20));

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildCrashed {
                    child_id: child,
                    reason: "test crash".to_string(),
                },
            )
            .expect("Failed to send");

        std::thread::sleep(Duration::from_millis(20));

        let handle = runtime
            .request(supervisor_id, SupervisorMessage::GetStatus)
            .expect("Failed to request");
        let reply = handle
            .recv_timeout(Duration::from_secs(1))
            .expect("Failed to receive");
        let statuses = reply
            .downcast::<Vec<ChildStatus>>()
            .expect("Failed to downcast");

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].restart_count, 1);
        assert!(statuses[0].alive);
    }

    #[test]
    fn test_supervisor_max_restarts() {
        let mut runtime = Runtime::new().expect("Failed to create runtime");
        let strategy = RestartStrategy::OneForOne {
            max_restarts: 2,
            within: Duration::from_secs(60),
            base_backoff: Duration::from_millis(0),
        };
        let supervisor = SupervisorActor::new(strategy);
        let supervisor_id = runtime
            .spawn(supervisor)
            .expect("Failed to spawn supervisor");
        let child = runtime
            .spawn(TestCounter { count: 0 })
            .expect("Failed to spawn child");

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildStarted {
                    child_id: child,
                    name: "persistent_crasher".to_string(),
                },
            )
            .expect("Failed to send");

        std::thread::sleep(Duration::from_millis(20));

        for i in 0..3 {
            runtime
                .send(
                    supervisor_id,
                    SupervisorMessage::ChildCrashed {
                        child_id: child,
                        reason: format!("crash {}", i),
                    },
                )
                .expect("Failed to send");
            std::thread::sleep(Duration::from_millis(20));
        }

        let handle = runtime
            .request(supervisor_id, SupervisorMessage::GetStatus)
            .expect("Failed to request");
        let reply = handle
            .recv_timeout(Duration::from_secs(1))
            .expect("Failed to receive");
        let statuses = reply
            .downcast::<Vec<ChildStatus>>()
            .expect("Failed to downcast");

        assert_eq!(statuses[0].restart_count, 3);
        assert!(!statuses[0].alive);
    }

    #[test]
    fn test_multiple_children_independent_restart_counts() {
        let mut runtime = Runtime::new().expect("Failed to create runtime");
        let strategy = RestartStrategy::OneForOne {
            max_restarts: 2,
            within: Duration::from_secs(60),
            base_backoff: Duration::from_millis(0),
        };
        let supervisor = SupervisorActor::new(strategy);
        let supervisor_id = runtime
            .spawn(supervisor)
            .expect("Failed to spawn supervisor");
        let child1 = runtime
            .spawn(TestCounter { count: 0 })
            .expect("Failed to spawn child1");
        let child2 = runtime
            .spawn(TestCounter { count: 0 })
            .expect("Failed to spawn child2");

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildStarted {
                    child_id: child1,
                    name: "child1".to_string(),
                },
            )
            .expect("Failed to send");

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildStarted {
                    child_id: child2,
                    name: "child2".to_string(),
                },
            )
            .expect("Failed to send");

        std::thread::sleep(Duration::from_millis(20));

        for _ in 0..3 {
            runtime
                .send(
                    supervisor_id,
                    SupervisorMessage::ChildCrashed {
                        child_id: child1,
                        reason: "child1 crash".to_string(),
                    },
                )
                .expect("Failed to send");
            std::thread::sleep(Duration::from_millis(20));
        }

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildCrashed {
                    child_id: child2,
                    reason: "child2 crash".to_string(),
                },
            )
            .expect("Failed to send");

        std::thread::sleep(Duration::from_millis(20));

        let handle = runtime
            .request(supervisor_id, SupervisorMessage::GetStatus)
            .expect("Failed to request");
        let reply = handle
            .recv_timeout(Duration::from_secs(1))
            .expect("Failed to receive");
        let statuses = reply
            .downcast::<Vec<ChildStatus>>()
            .expect("Failed to downcast");

        assert_eq!(statuses.len(), 2);

        let child1_status = statuses.iter().find(|s| s.name == "child1").unwrap();
        let child2_status = statuses.iter().find(|s| s.name == "child2").unwrap();

        assert_eq!(child1_status.restart_count, 3);
        assert!(!child1_status.alive);

        assert_eq!(child2_status.restart_count, 1);
        assert!(child2_status.alive);
    }

    #[test]
    fn test_supervisor_backoff_defers_restart() {
        let mut runtime = Runtime::new().expect("Failed to create runtime");
        let strategy = RestartStrategy::OneForOne {
            max_restarts: 5,
            within: Duration::from_secs(60),
            base_backoff: Duration::from_millis(200),
        };
        let supervisor = SupervisorActor::new(strategy);
        let supervisor_id = runtime
            .spawn(supervisor)
            .expect("Failed to spawn supervisor");
        let child = runtime
            .spawn(TestCounter { count: 0 })
            .expect("Failed to spawn child");

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildStarted {
                    child_id: child,
                    name: "backoff_child".to_string(),
                },
            )
            .expect("Failed to send");
        std::thread::sleep(Duration::from_millis(20));

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildCrashed {
                    child_id: child,
                    reason: "first crash".to_string(),
                },
            )
            .expect("Failed to send");
        std::thread::sleep(Duration::from_millis(20));

        let handle = runtime
            .request(supervisor_id, SupervisorMessage::GetStatus)
            .expect("Failed to request");
        let reply = handle
            .recv_timeout(Duration::from_secs(1))
            .expect("Failed to receive");
        let statuses = reply
            .downcast::<Vec<ChildStatus>>()
            .expect("Failed to downcast");
        assert!(
            statuses[0].alive,
            "first crash should restart immediately (no prior restart)"
        );

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildCrashed {
                    child_id: child,
                    reason: "second crash right after".to_string(),
                },
            )
            .expect("Failed to send");
        std::thread::sleep(Duration::from_millis(20));

        let handle = runtime
            .request(supervisor_id, SupervisorMessage::GetStatus)
            .expect("Failed to request");
        let reply = handle
            .recv_timeout(Duration::from_secs(1))
            .expect("Failed to receive");
        let statuses = reply
            .downcast::<Vec<ChildStatus>>()
            .expect("Failed to downcast");
        assert!(
            !statuses[0].alive,
            "second crash should be deferred by backoff"
        );

        std::thread::sleep(Duration::from_millis(1000));

        runtime
            .send(
                supervisor_id,
                SupervisorMessage::ChildCrashed {
                    child_id: child,
                    reason: "third crash after backoff".to_string(),
                },
            )
            .expect("Failed to send");
        std::thread::sleep(Duration::from_millis(20));

        let handle = runtime
            .request(supervisor_id, SupervisorMessage::GetStatus)
            .expect("Failed to request");
        let reply = handle
            .recv_timeout(Duration::from_secs(1))
            .expect("Failed to receive");
        let statuses = reply
            .downcast::<Vec<ChildStatus>>()
            .expect("Failed to downcast");
        assert!(statuses[0].alive, "should restart after backoff elapsed");
    }
}
