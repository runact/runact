# Runact Extensions

## Overview

The Runact extension system allows external components to interact with the runtime through a versioned, capability-controlled protocol. Extensions run as separate processes and communicate through message passing.

## Architecture

```
Runact
  │
  └── Extension Process
          │
      Versioned Protocol
```

## Extension Manifest

Every extension must provide a manifest.

### Manifest Format

```toml
name = "example"
version = "0.1.0"
runtime_api = "1"
description = "An example extension"
author = "Author Name"

[capabilities]
required = [
    "process.execute",
    "process.spawn",
    "system.read"
]

[process]
type = "external"  # or "embedded"
restart = "transient"
shutdown_timeout = "5s"
```

### Manifest Structure

```rust
pub struct Manifest {
    pub name: String,
    pub version: ProtocolVersion,
    pub runtime_api: ProtocolVersion,
    pub description: String,
    pub author: String,
    pub capabilities: CapabilityRequirements,
    pub process: ProcessConfig,
}

pub struct CapabilityRequirements {
    pub required: Vec<Capability>,
    pub optional: Vec<Capability>,
}

pub struct ProcessConfig {
    pub process_type: ProcessType,
    pub restart_policy: RestartPolicy,
    pub shutdown_timeout: Duration,
}

pub enum ProcessType {
    External,
    Embedded,
}
```

## Extension Process

Extensions run as supervised processes.

### ExtensionProcess

```rust
pub struct ExtensionProcess {
    id: ExtensionId,
    manifest: Manifest,
    state: ExtensionState,
    capabilities: Capabilities,
    protocol: ExtensionProtocol,
}

impl Process for ExtensionProcess {
    type Message = ExtensionMessage;

    async fn handle(
        &mut self,
        message: ExtensionMessage,
        context: &mut ProcessContext,
    ) -> Result<(), ProcessError> {
        match message {
            ExtensionMessage::Initialize(config) => {
                self.initialize(config, context).await
            }
            ExtensionMessage::Request(request) => {
                self.handle_request(request, context).await
            }
            ExtensionMessage::Event(event) => {
                self.handle_event(event, context).await
            }
            ExtensionMessage::Shutdown => {
                self.shutdown(context).await
            }
        }
    }
}
```

### Extension State

```rust
pub enum ExtensionState {
    Registered,
    Initializing,
    Running,
    Stopping,
    Stopped,
    Failed { error: String },
}
```

## Protocol

### Versioned Communication

All communication uses a versioned protocol.

```rust
pub struct ExtensionProtocol {
    local_version: ProtocolVersion,
    remote_version: ProtocolVersion,
    negotiated_version: ProtocolVersion,
}
```

### Protocol Messages

```rust
pub enum ExtensionMessage {
    Initialize(ExtensionConfig),
    Request(ExtensionRequest),
    Event(ExtensionEvent),
    Shutdown,
}

pub enum ExtensionRequest {
    // Process management
    ListProcesses,
    GetProcess { process_id: ProcessId },
    SpawnProcess { process_type: String, config: serde_json::Value },

    // Messaging
    SendMessage { target: ProcessId, message: serde_json::Value },

    // System
    QuerySystem,
    ExecuteCommand { command: String, args: serde_json::Value },
}

pub enum ExtensionResponse {
    Processes(Vec<ProcessInfo>),
    Process(ProcessInfo),
    ProcessSpawned(ProcessId),
    MessageSent { message_id: MessageId },
    SystemInfo(SystemInfo),
    CommandResult(serde_json::Value),
    Error(ExtensionError),
}

pub enum ExtensionEvent {
    ProcessStarted { process_id: ProcessId },
    ProcessStopped { process_id: ProcessId },
    ProcessFailed { process_id: ProcessId },
    MessageSent { message_id: MessageId },
}
```

### Handshake

```
Extension                          Runtime
    │                                 │
    │──── ExtensionHello ────────────▶│
    │     (version, capabilities)     │
    │                                 │
    │◀──── ExtensionWelcome ─────────│
    │     (supported_version,         │
    │      granted_capabilities)      │
    │                                 │
    │◀──── Ready ────────────────────│
    │                                 │
```

```rust
pub struct ExtensionHello {
    pub extension_id: ExtensionId,
    pub manifest: Manifest,
    pub protocol_version: ProtocolVersion,
}

pub struct ExtensionWelcome {
    pub runtime_id: RuntimeId,
    pub supported_version: ProtocolVersion,
    pub granted_capabilities: Capabilities,
    pub session_id: ExtensionSessionId,
}
```

## Lifecycle

### Registration

```rust
impl Runtime {
    pub async fn register_extension(&self, manifest: Manifest) -> Result<ExtensionId, ExtensionError> {
        // Validate manifest
        manifest.validate()?;

        // Check if capabilities are available
        self.check_extension_capabilities(&manifest)?;

        // Create extension process
        let extension = ExtensionProcess::new(manifest);

        // Start under supervisor
        let id = self.supervisor.spawn_child(extension)?;

        Ok(id)
    }
}
```

### Initialization

```rust
impl ExtensionProcess {
    async fn initialize(&mut self, config: ExtensionConfig, ctx: &mut ProcessContext) -> Result<(), ProcessError> {
        self.state = ExtensionState::Initializing;

        // Send hello to runtime
        let hello = ExtensionHello {
            extension_id: self.id,
            manifest: self.manifest.clone(),
            protocol_version: PROTOCOL_VERSION,
        };

        ctx.send_to_runtime(ExtensionMessage::Initialize(config));

        // Wait for welcome
        // ...

        self.state = ExtensionState::Running;
        Ok(())
    }
}
```

### Shutdown

```rust
impl ExtensionProcess {
    async fn shutdown(&mut self, ctx: &mut ProcessContext) -> Result<(), ProcessError> {
        self.state = ExtensionState::Stopping;

        // Notify extension
        ctx.send_to_extension(ExtensionMessage::Shutdown);

        // Wait for graceful shutdown
        let timeout = self.manifest.process.shutdown_timeout;
        tokio::time::timeout(timeout, self.wait_for_shutdown()).await?;

        self.state = ExtensionState::Stopped;
        Ok(())
    }
}
```

## Capability System

### Capability Negotiation

Extensions declare required capabilities. Runtime grants what it can.

```rust
impl Runtime {
    fn negotiate_capabilities(&self, manifest: &Manifest) -> Capabilities {
        let mut granted = Capabilities::new();

        for required in &manifest.capabilities.required {
            if self.can_grant_capability(required) {
                granted.grant(required.clone());
            } else {
                // Extension cannot start without this capability
                return Err(ExtensionError::CapabilityDenied {
                    capability: required.clone(),
                });
            }
        }

        for optional in &manifest.capabilities.optional {
            if self.can_grant_capability(optional) {
                granted.grant(optional.clone());
            }
        }

        granted
    }
}
```

### Runtime Capability Check

```rust
impl Runtime {
    fn can_grant_capability(&self, capability: &Capability) -> bool {
        // Check runtime policy
        self.policy.can_grant(capability)
    }
}
```

## External vs Embedded

### External Extensions

- Run in separate process
- Communicate via protocol
- Full isolation
- Can be written in any language
- Higher overhead

### Embedded Extensions

- Run in same process
- Direct function calls
- Lower overhead
- Rust-only
- Less isolation

```rust
pub enum ExtensionRuntime {
    External(ExternalExtension),
    Embedded(EmbeddedExtension),
}

pub trait EmbeddedExtension: Send + Sync + 'static {
    fn manifest(&self) -> &Manifest;
    fn handle_request(&mut self, request: ExtensionRequest) -> ExtensionResponse;
    fn handle_event(&mut self, event: ExtensionEvent);
}
```

## Error Handling

### Extension Errors

```rust
pub enum ExtensionError {
    ManifestInvalid { reason: String },
    CapabilityDenied { capability: Capability },
    ProtocolVersionMismatch { expected: ProtocolVersion, actual: ProtocolVersion },
    InitializationFailed { reason: String },
    RequestFailed { reason: String },
    ShutdownTimeout,
}
```

### Error Recovery

```rust
impl ExtensionProcess {
    async fn handle_error(&mut self, error: ExtensionError, ctx: &mut ProcessContext) {
        match error {
            ExtensionError::InitializationFailed { .. } => {
                // Mark as failed, supervisor will handle restart
                self.state = ExtensionState::Failed {
                    error: error.to_string(),
                };
            }
            ExtensionError::RequestFailed { .. } => {
                // Log error, continue running
                tracing::error!("Extension request failed: {}", error);
            }
            _ => {
                // Other errors
                tracing::warn!("Extension error: {}", error);
            }
        }
    }
}
```

## Discovery

### Extension Registry

```rust
pub struct ExtensionRegistry {
    extensions: HashMap<ExtensionId, ExtensionInfo>,
    manifest_paths: HashMap<ExtensionId, PathBuf>,
}

pub struct ExtensionInfo {
    pub id: ExtensionId,
    pub manifest: Manifest,
    pub state: ExtensionState,
    pub capabilities: Capabilities,
}
```

### Discovery Paths

Extensions are discovered from:

1. `~/.config/runact/extensions/`
2. `<workspace>/.runact/extensions/`
3. `<project>/.runact/extensions/`

```rust
impl ExtensionRegistry {
    pub fn discover() -> Self {
        let mut registry = Self::new();

        // Search extension paths
        for path in extension_paths() {
            if let Ok(extensions) = discover_extensions(&path) {
                for ext in extensions {
                    registry.register(ext);
                }
            }
        }

        registry
    }
}
```

## Security

### Extension Isolation

- Extensions run in separate processes
- No shared memory
- Communication via protocol only
- Capability-checked

### Capability Logging

All extension capability usage is logged:

```rust
pub struct ExtensionAuditEntry {
    pub extension_id: ExtensionId,
    pub capability: Capability,
    pub resource: ResourceId,
    pub timestamp: Timestamp,
    pub result: AuditResult,
}
```

## Observability

### Extension Metrics

```rust
pub struct ExtensionMetrics {
    pub request_count: u64,
    pub request_latency: Duration,
    pub error_count: u64,
    pub event_count: u64,
    pub capability_denials: u64,
}
```

### Health Checks

```rust
impl ExtensionProcess {
    pub async fn health_check(&self) -> HealthStatus {
        match self.state {
            ExtensionState::Running => HealthStatus::Healthy,
            ExtensionState::Failed { .. } => HealthStatus::Unhealthy,
            _ => HealthStatus::Degraded,
        }
    }
}
```
