use std::time::Duration;

use runact::{Actor, ActorContext, ActorError, Runtime, RuntimeStats};

struct PingActor;

enum PingMessage {
    Ping,
}

impl Actor for PingActor {
    type Message = PingMessage;

    fn handle(
        &mut self,
        msg: PingMessage,
        ctx: &mut ActorContext,
    ) -> Result<(), ActorError> {
        match msg {
            PingMessage::Ping => {
                ctx.reply("pong".to_string()).ok();
            }
        }
        Ok(())
    }
}

#[test]
fn test_stats_initial() {
    let mut runtime = Runtime::new().expect("create runtime");
    let _ = runtime.spawn(PingActor).expect("spawn");
    let _ = runtime.spawn(PingActor).expect("spawn");

    let stats: RuntimeStats = runtime.stats();
    assert_eq!(stats.actor_count, 2);
    assert_eq!(stats.request_count, 0);
}

#[test]
fn test_stats_requests_increment_counter() {
    let mut runtime = Runtime::new().expect("create runtime");
    let id = runtime.spawn(PingActor).expect("spawn");

    for _ in 0..5 {
        let handle = runtime.request(id, PingMessage::Ping).expect("request");
        let _ = handle.recv_timeout(Duration::from_millis(100));
    }

    let stats: RuntimeStats = runtime.stats();
    assert_eq!(stats.actor_count, 1);
    assert_eq!(stats.request_count, 5);
}

#[test]
fn test_stats_send_does_not_increment_request_count() {
    let mut runtime = Runtime::new().expect("create runtime");
    let id = runtime.spawn(PingActor).expect("spawn");

    let _ = runtime.send(id, PingMessage::Ping);
    let _ = runtime.send(id, PingMessage::Ping);

    let stats: RuntimeStats = runtime.stats();
    assert_eq!(stats.request_count, 0);
}

#[test]
fn test_stats_decreasing_actor_count() {
    let mut runtime = Runtime::new().expect("create runtime");
    let _ = runtime.spawn(PingActor).expect("spawn");
    let _ = runtime.spawn(PingActor).expect("spawn");
    let _ = runtime.spawn(PingActor).expect("spawn");

    assert_eq!(runtime.stats().actor_count, 3);

    runtime.shutdown().expect("shutdown");
    let stats: RuntimeStats = runtime.stats();
    assert_eq!(stats.actor_count, 0);
}
