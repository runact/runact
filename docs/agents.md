# Runact Agents

## Overview

Agents in Runact are runtime entities that interact with the system through a controlled, auditable interface. An agent is NOT the LLM itself — it is a process that wraps an LLM adapter and executes operations through the standard message system.

## Architecture

```
Agent
├── Identity
├── Capabilities
├── Context
├── Task
├── State
├── Tools
├── Audit
└── ModelAdapter
```

The LLM is replaceable. Future adapters:

- Local model
- OpenAI-compatible API
- Anthropic-compatible API
- Ollama
- Custom inference
- Future models

## Agent Process

An agent is implemented as a supervised process.

```rust
pub struct AgentProcess {
    id: AgentId,
    name: String,
    adapter: Box<dyn ModelAdapter>,
    capabilities: Capabilities,
    context: AgentContext,
    state: AgentState,
    audit_log: AuditLog,
}
```

### Process Implementation

```rust
impl Process for AgentProcess {
    type Message = AgentMessage;

    async fn handle(
        &mut self,
        message: AgentMessage,
        context: &mut ProcessContext,
    ) -> Result<(), ProcessError> {
        match message {
            AgentMessage::ExecuteTask(task) => {
                self.execute_task(task, context).await
            }
            AgentMessage::ReceiveObservation(obs) => {
                self.process_observation(obs, context).await
            }
            AgentMessage::RequestApproval(tx_id) => {
                self.handle_approval(tx_id, context).await
            }
        }
    }
}
```

## Identity

```rust
pub struct AgentIdentity {
    pub id: AgentId,
    pub name: String,
    pub description: String,
    pub created_at: Timestamp,
}
```

## Capabilities

Capability-based permissions control what agents can do.

### Capability Set

```rust
pub struct Capabilities {
    pub grants: Vec<Capability>,
}
```

### Capability Types

```rust
pub enum Capability {
    ProcessExecute,
    ProcessSpawn,
    TimerCreate,
    SystemRead,
    SystemWrite,
    NetworkConnect,
}
```

### Capability Checking

```rust
impl Capabilities {
    pub fn has(&self, capability: &Capability) -> bool {
        self.grants.iter().any(|g| g.matches(capability))
    }

    pub fn require(&self, capability: &Capability) -> Result<(), CapabilityError> {
        if self.has(capability) {
            Ok(())
        } else {
            Err(CapabilityError::Missing {
                required: capability.clone(),
            })
        }
    }
}
```

## Context

Agent maintains context about its current state.

```rust
pub struct AgentContext {
    pub history: Vec<AgentAction>,
    pub observations: Vec<Observation>,
}
```

### AgentAction

```rust
pub struct AgentAction {
    pub action_id: MessageId,
    pub action_type: ActionType,
    pub timestamp: Timestamp,
    pub result: ActionResult,
}

pub enum ActionType {
    SendMessage { target: ProcessId },
    SpawnProcess { process_type: String },
    CreateTimer { duration: Duration },
    ExecuteCommand { command_name: String },
}

pub enum ActionResult {
    Success { output: String },
    Failure { error: String },
    RequiresApproval { request_id: MessageId },
}
```

### Observation

```rust
pub struct Observation {
    pub observation_id: MessageId,
    pub source: ProcessId,
    pub observation_type: ObservationType,
    pub timestamp: Timestamp,
    pub data: Vec<u8>,
}

pub enum ObservationType {
    ProcessStarted { process_id: ProcessId },
    ProcessStopped { process_id: ProcessId },
    MessageReceived { message_id: MessageId },
    TimerFired { timer_id: TimerId },
}
```

## State

```rust
pub enum AgentState {
    Idle,
    Thinking { task: Task },
    Executing { action: ActionType },
    WaitingForApproval { request_id: MessageId },
    WaitingForObservation,
    Failed { error: String },
}
```

## Model Adapter

Interface for LLM integration.

```rust
#[async_trait]
pub trait ModelAdapter: Send + Sync {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Completion, ModelError>;

    async fn embedding(&self, text: &str) -> Result<Vec<f32>, ModelError>;

    fn model_name(&self) -> &str;
    fn supports_tools(&self) -> bool;
}
```

### Message

```rust
pub struct Message {
    pub role: Role,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}
```

### Tool Definition

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: JsonSchema,
}
```

### Completion

```rust
pub struct Completion {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: FinishReason,
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
}
```

## Tool API

Agents interact with the system through tools.

### Built-in Tools

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        registry.register(SendMessageTool);
        registry.register(SpawnProcessTool);
        registry.register(CreateTimerTool);
        registry.register(ListProcessesTool);
        registry.register(InspectProcessTool);
        registry
    }
}
```

### Tool Trait

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> JsonSchema;
    fn required_capabilities(&self) -> Vec<Capability>;

    async fn execute(
        &self,
        arguments: serde_json::Value,
        context: &ToolContext,
    ) -> Result<serde_json::Value, ToolError>;
}
```

### Tool Context

```rust
pub struct ToolContext {
    pub agent_id: AgentId,
    pub capabilities: Capabilities,
    pub runtime: RuntimeHandle,
}
```

### Tool Examples

```rust
pub struct SendMessageTool;

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str { "process.send" }
    fn description(&self) -> &str { "Send a message to a process" }

    fn required_capabilities(&self) -> Vec<Capability> {
        vec![Capability::ProcessExecute]
    }

    fn parameters(&self) -> JsonSchema {
        json!({
            "type": "object",
            "properties": {
                "target": { "type": "string" },
                "message": { "type": "object" }
            },
            "required": ["target", "message"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<serde_json::Value, ToolError> {
        let target: ProcessId = serde_json::from_value(args["target"])?;
        let message: serde_json::Value = args["message"].clone();

        // Check capability
        ctx.capabilities.require(&Capability::ProcessExecute)?;

        // Send message
        ctx.runtime.send(target, message).await?;

        Ok(json!({ "success": true }))
    }
}
```

## Task Execution

### Task

```rust
pub struct Task {
    pub task_id: MessageId,
    pub description: String,
    pub goal: String,
    pub constraints: Vec<String>,
    pub max_iterations: usize,
}
```

### Execution Loop

```rust
impl AgentProcess {
    async fn execute_task(&mut self, task: Task, ctx: &mut ProcessContext) -> Result<(), ProcessError> {
        self.state = AgentState::Thinking { task: task.clone() };

        let mut messages = vec![
            Message {
                role: Role::System,
                content: self.system_prompt(),
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: Role::User,
                content: task.description.clone(),
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        for iteration in 0..task.max_iterations {
            // Get completion from model
            let completion = self.adapter.complete(&messages, &self.tool_definitions()).await?;

            // Handle tool calls
            if !completion.tool_calls.is_empty() {
                for tool_call in &completion.tool_calls {
                    // Check capabilities
                    let tool = self.tools.get(&tool_call.name)?;
                    ctx.capabilities.require(&tool.required_capabilities())?;

                    // Execute tool
                    self.state = AgentState::Executing {
                        action: ActionType::ExecuteCommand {
                            command_name: tool_call.name.clone(),
                        },
                    };

                    let result = tool.execute(tool_call.arguments.clone(), &self.tool_context(ctx)).await;

                    // Record action
                    self.context.history.push(AgentAction {
                        action_id: MessageId::new(),
                        action_type: ActionType::ExecuteCommand {
                            command_name: tool_call.name.clone(),
                        },
                        timestamp: Timestamp::now(),
                        result: match &result {
                            Ok(_) => ActionResult::Success {
                                output: serde_json::to_string(&result)?,
                            },
                            Err(e) => ActionResult::Failure {
                                error: e.to_string(),
                            },
                        },
                    });

                    // Add tool result to messages
                    messages.push(Message {
                        role: Role::Tool,
                        content: serde_json::to_string(&result)?,
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                }
            } else {
                // No tool calls — task complete
                if let Some(content) = completion.content {
                    // Output final response
                    ctx.send_response(content);
                }
                break;
            }
        }

        self.state = AgentState::Idle;
        Ok(())
    }
}
```

## Approval Flow

### Dangerous Operations

Some operations require explicit approval.

```rust
impl AgentProcess {
    async fn spawn_process(&mut self, config: ProcessConfig, ctx: &mut ProcessContext) -> Result<(), AgentError> {
        // Check capability
        self.capabilities.require(&Capability::ProcessSpawn)?;

        // Check if approval required
        if self.requires_approval(&config) {
            self.state = AgentState::WaitingForApproval {
                request_id: MessageId::new(),
            };

            // Request approval
            // ... wait for user decision
        } else {
            // Auto-approve
            ctx.runtime.spawn(config).await?;
        }

        Ok(())
    }

    fn requires_approval(&self, config: &ProcessConfig) -> bool {
        // Check if process type is sensitive
        config.sensitive
    }
}
```

## Audit

Every agent action is recorded.

```rust
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

pub struct AuditEntry {
    pub entry_id: MessageId,
    pub agent_id: AgentId,
    pub action: AuditAction,
    pub timestamp: Timestamp,
    pub context: AuditContext,
    pub result: AuditResult,
}

pub enum AuditAction {
    SendMessage { target: ProcessId },
    SpawnProcess { process_type: String },
    CreateTimer { duration: Duration },
    ExecuteCommand { command: String },
}

pub struct AuditContext {
    pub correlation_id: Option<CorrelationId>,
}

pub enum AuditResult {
    Success,
    Failure { error: String },
    Denied { capability: Capability },
}
```

## Observability

### Agent Metrics

```rust
pub struct AgentMetrics {
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub tool_calls: u64,
    pub messages_sent: u64,
    pub processes_spawned: u64,
    pub approvals_requested: u64,
    pub approvals_granted: u64,
    pub approvals_denied: u64,
    pub average_task_duration: Duration,
}
```

### Tracing

```rust
#[instrument(skip(self, ctx), fields(agent_id = %self.id))]
async fn execute_task(&mut self, task: Task, ctx: &mut ProcessContext) -> Result<(), ProcessError> {
    // ...
}
```
