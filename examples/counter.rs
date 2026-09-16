use runact::{Actor, ActorContext, ActorError, Runtime};

/// Simple counter actor.
struct CounterActor {
    count: u64,
}

enum CounterMessage {
    _Increment,
    _Decrement,
    _GetValue,
}

impl Actor for CounterActor {
    type Message = CounterMessage;

    fn handle(&mut self, msg: CounterMessage, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            CounterMessage::_Increment => {
                self.count += 1;
                tracing::info!(actor_id = %ctx.actor_id(), count = self.count, "Incremented");
            }
            CounterMessage::_Decrement => {
                self.count = self.count.saturating_sub(1);
                tracing::info!(actor_id = %ctx.actor_id(), count = self.count, "Decremented");
            }
            CounterMessage::_GetValue => {
                tracing::info!(actor_id = %ctx.actor_id(), count = self.count, "GetValue");
            }
        }
        Ok(())
    }
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting Runact runtime");

    // Create runtime
    let mut runtime = Runtime::new().expect("Failed to create runtime");

    // Spawn actors
    let actor1 = runtime
        .spawn(CounterActor { count: 0 })
        .expect("Failed to spawn actor1");
    let actor2 = runtime
        .spawn(CounterActor { count: 10 })
        .expect("Failed to spawn actor2");

    tracing::info!(actor1 = %actor1, actor2 = %actor2, "Actors spawned");

    // List actors
    let actors = runtime.list_actors().expect("Failed to list actors");
    tracing::info!(count = actors.len(), "Active actors");

    // Shutdown
    runtime.shutdown().expect("Failed to shutdown");
    tracing::info!("Runtime shut down");
}
