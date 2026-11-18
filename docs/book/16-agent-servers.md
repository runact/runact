# Chapter 16: Agent Servers

This chapter walks through the `agent_server.rs` example: a WebSocket
server that bridges runact's TCP listener to a supervised `AgentActor`
with a pluggable LLM adapter.

---

## 16.1 Architecture

```
WebSocket Client
    │ (WebSocket over TCP)
    ▼
WebSocketServer (runact-web)
    │ (ServerEvent: Frame)
    ▼
ConnectionWriter callback
    │ (RuntimeSender::send)
    ▼
AgentActor (runact)
    │
    ├── ModelAdapter (trait)
    │     └── MockAdapter (example)
    │
    └── ActorContext
          ├── spawn_compute (CPU work)
          ├── schedule_timer (delays)
          └── send_to (inter-actor messaging)
```

## 16.2 The Agent Message Protocol

```rust
enum AgentMessage {
    UserInput(String),       // User sent a message
    LLMResponse(String),     // Adapter returned a response
    Error(String),            // Error from the adapter
}
```

## 16.3 The ModelAdapter Trait

```rust
trait ModelAdapter: Send + 'static {
    fn generate(&self, prompt: &str) -> String;
}
```

This is the pluggable interface. The example provides a `MockAdapter`
that simulates an LLM:

```rust
struct MockAdapter;

impl ModelAdapter for MockAdapter {
    fn generate(&self, prompt: &str) -> String {
        format!("Mock response to: {}", prompt)
    }
}
```

## 16.4 The AgentActor

```rust
struct AgentActor {
    adapter: std::sync::Arc<dyn ModelAdapter>,
    runtime_sender: RuntimeSender,
}

impl Actor for AgentActor {
    type Message = AgentMessage;

    fn handle(&mut self, msg: AgentMessage, ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        match msg {
            AgentMessage::UserInput(input) => {
                // Offload LLM call to compute pool. Clone the Arc so the
                // closure is 'static (it cannot borrow `self`).
                let adapter = std::sync::Arc::clone(&self.adapter);
                let sender = self.runtime_sender.clone();
                let conn_id = ctx.actor_id();
                ctx.spawn_compute(move || {
                    let response = adapter.generate(&input);
                    // Forward response back to this actor
                    sender.send(conn_id, AgentMessage::LLMResponse(response))
                        .ok();
                })
                .map_err(|e| ActorError::Handler(e.to_string()))?;
            }
            AgentMessage::LLMResponse(response) => {
                // Send response to the WebSocket client
                // (The WebSocket callback sends this to the actor,
                //  and here we'd forward it back through a writer thread)
                println!("Agent response: {}", response);
            }
            AgentMessage::Error(e) => {
                eprintln!("Agent error: {}", e);
            }
        }
        Ok(())
    }
}
```

## 16.5 Bridging WebSocket to Actors

Each WebSocket connection gets its own `AgentActor` instance. Accept the
connection first, then hand the stream to `WebSocketServer::bind`:

```rust
use runact_web::websocket::async_ws::WebSocketConfig;
use runact_web::websocket::server::{ServerEvent, WebSocketServer};

let listener = std::net::TcpListener::bind("127.0.0.1:8080")?;
let runtime_sender = runtime.sender();
// Spawn the agent actor(s) up front; only spawning needs `&mut Runtime`.
let agent_id = runtime.spawn(AgentActor {
    adapter: std::sync::Arc::new(MockAdapter),
    runtime_sender: runtime_sender.clone(),
})?;

for stream in listener.incoming() {
    let stream = stream?;
    let sender = runtime_sender.clone();

    std::thread::spawn(move || {
        let server = match WebSocketServer::bind(stream, "/ws") {
            Ok(server) => server,
            Err(e) => {
                eprintln!("handshake failed: {e}");
                return;
            }
        };

        let _ = server.accept_with_callback_and_config(
            |_writer, event| match event {
                ServerEvent::Connected => {
                    // Actor spawning requires &mut Runtime, so agents are
                    // pre-spawned; RuntimeSender is for messaging only.
                }
                ServerEvent::Frame(frame) => {
                    let text = String::from_utf8_lossy(&frame.payload);
                    let _ = sender
                        .send(agent_id, AgentMessage::UserInput(text.to_string()));
                }
                ServerEvent::Closed => { /* cleanup */ }
                ServerEvent::Error(e) => eprintln!("error: {e}"),
            },
            WebSocketConfig::default(),
        );
    });
}
```

### Key Patterns

1. **RuntimeSender** is `Clone` — pass it to each connection thread.
2. **Per-connection actors** — each WebSocket client gets its own
   `AgentActor` with isolated state.
3. **Compute pool offloading** — LLM calls run on compute workers, not
   the scheduler.
4. **Non-blocking communication** — `RuntimeSender::send` is `try_send`;
   the actor processes messages from its mailbox.

## 16.6 Full Example

See `examples/agent_server.rs` for the complete implementation. The example:

```rust
use runact_web::websocket::async_ws::WebSocketConfig;
use runact_web::websocket::server::WebSocketServer;

fn main() {
    let mut runtime = Runtime::new().unwrap();
    let sender = runtime.sender();

    let listener = std::net::TcpListener::bind("127.0.0.1:8080").unwrap();
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let sender = sender.clone();
        std::thread::spawn(move || {
            let server = match WebSocketServer::bind(stream, "/ws") {
                Ok(server) => server,
                Err(e) => {
                    eprintln!("handshake failed: {e}");
                    return;
                }
            };
            let _ = server.accept_with_callback_and_config(
                |_writer, event| { /* forward messages to the actor via `sender` */ },
                WebSocketConfig { ping_interval: Some(Duration::from_secs(30)) },
            );
        });
    }
}
```

## 16.7 Testing

The `ws_agent_server.rs` integration test verifies:
- WebSocket handshake succeeds (HTTP 101 response)
- Message forwarding from client → actor works end-to-end

## 16.8 Production Considerations

- Replace `MockAdapter` with a real LLM client (OpenAI, Anthropic, etc.)
- Add message history and context window management
- Add rate limiting and token budgeting
- Use `supervisor` for automatic restart on crash
- Add graceful shutdown for in-flight requests
