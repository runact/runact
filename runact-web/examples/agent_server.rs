//! AI agent server example.
//!
//! A WebSocket server that accepts agent sessions. Each client connection
//! is handled by a callback that forwards user messages to a supervised
//! runact actor. The actor wraps a pluggable LLM adapter.
//!
//! The LLM adapter is pluggable — this example uses a `MockAdapter` that
//! echoes messages with a prefix, demonstrating the full lifecycle without
//! requiring an external API key.
//!
//! Architecture:
//! ```text
//! TCP Listener
//!    └── WebSocketServer::bind  (runact TcpStream via adapter)
//!         └── accept_with_callback_and_config
//!              └── callback: WebSocket ↔ AgentActor via RuntimeSender
//!                   └── AgentActor (runact Actor + MockModelAdapter)
//! ```
//!
//! Run:
//! ```sh
//! cargo run -p runact-web --example agent_server
//! ```
//!
//! Test with websocat:
//! ```sh
//! websocat ws://127.0.0.1:8080/agent
//! ```

use runact::Runtime;
use runact::actor::{Actor, ActorContext, ActorError, ActorId};
use runact_web::websocket::async_ws::WebSocketConfig;
use runact_web::websocket::frame::OpCode;
use runact_web::websocket::runact_tcp::RunactTcpStream;
use runact_web::websocket::server::{ServerEvent, WebSocketServer};
use std::thread;
use std::time::Duration;

/// A message delivered to the agent actor.
#[derive(Debug, Clone)]
enum AgentMessage {
    UserMessage(String),
}

/// Pluggable LLM adapter trait.
trait ModelAdapter: Send + 'static {
    fn generate(&self, prompt: &str) -> String;
}

/// Mock adapter that echoes messages with a prefix.
struct MockAdapter;
impl ModelAdapter for MockAdapter {
    fn generate(&self, prompt: &str) -> String {
        format!("[Agent] You said: {prompt}")
    }
}

/// Agent actor — manages conversation state and delegates to LLM adapter.
struct AgentActor {
    #[allow(dead_code)]
    id: ActorId,
    name: String,
    adapter: Box<dyn ModelAdapter>,
    history: Vec<String>,
}

impl AgentActor {
    fn new(id: ActorId, name: String, adapter: Box<dyn ModelAdapter>) -> Self {
        eprintln!("agent_server: agent '{name}' (id={id:?}) created");
        AgentActor {
            id,
            name,
            adapter,
            history: Vec::new(),
        }
    }
}

impl Actor for AgentActor {
    type Message = AgentMessage;

    fn handle(&mut self, msg: Self::Message, _ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            AgentMessage::UserMessage(text) => {
                eprintln!("agent_server: agent '{}' received: {text}", self.name);
                self.history.push(text.clone());
                let response = self.adapter.generate(&text);
                self.history.push(response.clone());
                eprintln!("agent_server: agent '{}' responding: {response}", self.name);
                println!("{response}");
            }
        }
        Ok(())
    }
}

fn main() {
    let listener = runact::net::tcp_api::TcpListener::bind("127.0.0.1:8080").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    eprintln!("agent_server: listening on {addr}");
    eprintln!("agent_server: connect at ws://{addr}/agent");

    // Create the runact runtime and spawn the shared agent actor
    let mut runtime = Runtime::new().expect("runtime");
    eprintln!("agent_server: runtime created");

    let agent_id = runtime
        .spawn(AgentActor::new(
            ActorId::new(1),
            "echo-agent".to_string(),
            Box::new(MockAdapter),
        ))
        .expect("spawn agent");
    eprintln!("agent_server: spawned shared agent actor {agent_id:?}");

    // Runtime must outlive all spawned actors; the scheduler threads
    // are owned by the runtime.
    let _rt = runtime;

    // RuntimeSender is Clone — can be passed to I/O threads
    let sender = _rt.sender();

    loop {
        let stream = match listener.accept() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("agent_server: accept error: {e}");
                continue;
            }
        };

        let sender = sender.clone();
        thread::spawn(move || {
            // Wrap runact TcpStream in adapter for WebSocketServer
            let adapter = RunactTcpStream(stream);

            let server = match WebSocketServer::bind(adapter, "/agent") {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("agent_server: handshake error: {e}");
                    return;
                }
            };

            let config = WebSocketConfig {
                ping_interval: Some(Duration::from_secs(30)),
            };

            if let Err(e) = server.accept_with_callback_and_config(
                move |writer, event| match event {
                    ServerEvent::Connected => {
                        let _ = writer.send_text("Welcome to the agent server. Type your message.");
                    }
                    ServerEvent::Frame(f) => {
                        if f.opcode == OpCode::Text {
                            let text = String::from_utf8_lossy(&f.payload).to_string();
                            eprintln!("agent_server: client message: {text}");

                            // Forward to the agent actor for processing
                            let _ = sender.send(agent_id, AgentMessage::UserMessage(text.clone()));

                            // Echo back immediately (synchronous callback —
                            // in a real app, the actor would process and
                            // the response would come back asynchronously)
                            let response = format!("[Echo] {text}");
                            let _ = writer.send_text(&response);
                        }
                    }
                    ServerEvent::Closed => {
                        eprintln!("agent_server: client disconnected");
                    }
                    ServerEvent::Error(e) => {
                        eprintln!("agent_server: connection error: {e}");
                    }
                },
                config,
            ) {
                eprintln!("agent_server: handshake failed: {e}");
            }
        });
    }
}
