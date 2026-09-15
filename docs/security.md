# Runact Security

## Overview

Security in Runact is based on capabilities. Every operation that could affect the system requires explicit permission. No implicit trust is granted to any component, including AI agents.

## Core Principles

1. **Least privilege** — Grant only what is needed
2. **Explicit permissions** — No implicit trust
3. **Audit everything** — Record all significant operations
4. **Fail secure** — Deny by default
5. **Defense in depth** — Multiple layers of protection

## Capability System

### Capability

A capability is a permission unit.

```rust
pub enum Capability {
    // Process
    ProcessExecute,
    ProcessSpawn,

    // System
    SystemRead,
    SystemWrite,

    // Network
    NetworkConnect { hosts: Vec<String> },

    // Extension
    ExtensionRegister,
    ExtensionExecute,

    // Timer
    TimerCreate,
}
```

### Capability Set

A collection of capabilities granted to a principal.

```rust
pub struct Capabilities {
    grants: Vec<Capability>,
}

impl Capabilities {
    pub fn new() -> Self {
        Self { grants: Vec::new() }
    }

    pub fn grant(&mut self, capability: Capability) {
        self.grants.push(capability);
    }

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

### Capability Matching

```rust
impl Capability {
    fn matches(&self, required: &Capability) -> bool {
        match (self, required) {
            (Capability::ProcessExecute, Capability::ProcessExecute) => true,
            (Capability::ProcessSpawn, Capability::ProcessSpawn) => true,
            (Capability::SystemRead, Capability::SystemRead) => true,
            (Capability::SystemWrite, Capability::SystemWrite) => true,
            (Capability::NetworkConnect { hosts }, Capability::NetworkConnect { hosts: required }) => {
                hosts.iter().any(|h| required.iter().any(|r| r == h))
            }
            (Capability::ExtensionRegister, Capability::ExtensionRegister) => true,
            (Capability::ExtensionExecute, Capability::ExtensionExecute) => true,
            (Capability::TimerCreate, Capability::TimerCreate) => true,
            _ => false,
        }
    }
}
```

## Principal Types

### Process

Every process has capabilities.

```rust
pub struct Process {
    id: ProcessId,
    capabilities: Capabilities,
    // ...
}
```

### Agent

Agents have explicitly granted capabilities.

```rust
pub struct Agent {
    id: AgentId,
    capabilities: Capabilities,
    // ...
}
```

### Extension

Extensions declare required capabilities.

```rust
pub struct Extension {
    id: ExtensionId,
    manifest: Manifest,
    granted_capabilities: Capabilities,
}

pub struct Manifest {
    name: String,
    version: ProtocolVersion,
    required_capabilities: Vec<Capability>,
}
```

## Policy

### Policy Rules

```rust
pub struct Policy {
    rules: Vec<PolicyRule>,
}

pub enum PolicyRule {
    AlwaysAllow { capability: Capability },
    AlwaysDeny { capability: Capability },
    AllowIf { capability: Capability, condition: PolicyCondition },
    RequireApproval { capability: Capability, approver: ApprovalSource },
}

pub enum PolicyCondition {
    TimeRange { start: Time, end: Time },
    PathPattern { pattern: String },
    ProcessType { process_type: String },
    AgentId { agent_id: AgentId },
}
```

### Policy Evaluation

```rust
impl Policy {
    pub fn evaluate(&self, capability: &Capability, context: &PolicyContext) -> PolicyDecision {
        for rule in &self.rules {
            match rule {
                PolicyRule::AlwaysAllow { capability: c } if c.matches(capability) => {
                    return PolicyDecision::Allow;
                }
                PolicyRule::AlwaysDeny { capability: c } if c.matches(capability) => {
                    return PolicyDecision::Deny;
                }
                PolicyRule::RequireApproval { capability: c, .. } if c.matches(capability) => {
                    return PolicyDecision::RequireApproval;
                }
                _ => {}
            }
        }

        PolicyDecision::Deny  // Default: deny
    }
}

pub enum PolicyDecision {
    Allow,
    Deny,
    RequireApproval,
}
```

## Approval System

### Approval Request

```rust
pub struct ApprovalRequest {
    pub request_id: MessageId,
    pub principal_id: PrincipalId,
    pub capability: Capability,
    pub context: ApprovalContext,
    pub timestamp: Timestamp,
}

pub struct ApprovalContext {
    pub description: String,
    pub risk_level: RiskLevel,
    pub affected_resources: Vec<ResourceId>,
    pub preview: Option<String>,
}

pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
```

### Approval Response

```rust
pub struct ApprovalResponse {
    pub request_id: MessageId,
    pub decision: ApprovalDecision,
    pub approver: ApproverId,
    pub timestamp: Timestamp,
    pub reason: Option<String>,
}

pub enum ApprovalDecision {
    Approve,
    Deny,
    ApproveWithConditions { conditions: Vec<String> },
}
```

### Approval Flow

```
Principal requests capability
        │
        ▼
Policy evaluation
        │
    ┌───┴───┐
    ▼       ▼
 Allow   Require Approval
    │       │
    │       ▼
    │    Show approval request
    │       │
    │       ▼
    │    User/system decides
    │       │
    │   ┌───┴───┐
    │   ▼       ▼
    │ Approve  Deny
    │   │       │
    ▼   ▼       ▼
Execute      Reject
```

## Default Policies

### Runtime Defaults

```rust
impl Default for Policy {
    fn default() -> Self {
        Self {
            rules: vec![
                // Always allow read operations
                PolicyRule::AlwaysAllow { capability: Capability::SystemRead },

                // Require approval for write operations from agents
                PolicyRule::RequireApproval {
                    capability: Capability::SystemWrite,
                    approver: ApprovalSource::User,
                },

                // Deny dangerous operations by default
                PolicyRule::AlwaysDeny { capability: Capability::NetworkConnect { hosts: vec![] } },
                PolicyRule::AlwaysDeny { capability: Capability::ExtensionExecute },
            ],
        }
    }
}
```

### Extension Defaults

Extensions start with minimal capabilities:

```rust
impl Extension {
    fn default_capabilities() -> Capabilities {
        let mut caps = Capabilities::new();
        caps.grant(Capability::WorkspaceRead);
        caps
    }
}
```

## Resource Protection

### Protected Resources

```rust
pub enum ResourceId {
    Buffer(BufferId),
    File(PathBuf),
    Process(ProcessId),
    Workspace(WorkspaceId),
    Project(ProjectId),
}
```

### Access Control

```rust
impl Runtime {
    pub fn check_access(
        &self,
        principal: PrincipalId,
        resource: ResourceId,
        operation: Operation,
    ) -> Result<(), AccessError> {
        let capabilities = self.get_capabilities(principal)?;
        let required = self.required_capability(resource, operation)?;

        capabilities.require(&required)
    }
}
```

## Audit

### Security Audit Log

```rust
pub struct SecurityAuditLog {
    entries: Vec<SecurityAuditEntry>,
}

pub struct SecurityAuditEntry {
    pub entry_id: MessageId,
    pub timestamp: Timestamp,
    pub principal_id: PrincipalId,
    pub capability: Capability,
    pub resource: ResourceId,
    pub decision: PolicyDecision,
    pub context: String,
}
```

### Audit Queries

```rust
impl SecurityAuditLog {
    pub fn query_by_principal(&self, id: PrincipalId) -> Vec<&SecurityAuditEntry>;
    pub fn query_by_capability(&self, cap: &Capability) -> Vec<&SecurityAuditEntry>;
    pub fn query_by_time(&self, start: Timestamp, end: Timestamp) -> Vec<&SecurityAuditEntry>;
    pub fn query_denials(&self) -> Vec<&SecurityAuditEntry>;
}
```

## Threat Model

### Threats

1. **Malicious agent** — Agent attempts unauthorized operations
2. **Malicious extension** — Extension attempts to escalate privileges
3. **Compromised process** — Process attempts to access unauthorized resources
4. **Social engineering** — User tricked into granting excessive permissions

### Mitigations

1. **Capability enforcement** — All operations checked
2. **Audit logging** — All operations recorded
3. **Approval gates** — Dangerous operations require explicit approval
4. **Process isolation** — Processes cannot access each other's state directly
5. **Extension sandboxing** — Extensions run with minimal capabilities

## Configuration

### Security Configuration

```rust
pub struct SecurityConfig {
    pub policy: Policy,
    pub audit_retention: Duration,
    pub max_capabilities_per_agent: usize,
    pub require_approval_above_risk: RiskLevel,
}
```

### Runtime Security Setup

```rust
impl Runtime {
    pub fn with_security_config(config: SecurityConfig) -> Self {
        let mut runtime = Self::new();
        runtime.policy = config.policy;
        runtime.audit_log = SecurityAuditLog::new(config.audit_retention);
        runtime
    }
}
```

## Observability

### Security Metrics

```rust
pub struct SecurityMetrics {
    pub access_checks: u64,
    pub access_granted: u64,
    pub access_denied: u64,
    pub approvals_requested: u64,
    pub approvals_granted: u64,
    pub approvals_denied: u64,
    pub audit_entries: u64,
}
```

### Alerting

```rust
pub enum SecurityAlert {
    ExcessiveDenials { principal_id: PrincipalId, count: usize },
    UnauthorizedAccessAttempt { principal_id: PrincipalId, capability: Capability },
    CapabilityEscalation { principal_id: PrincipalId, from: Capability, to: Capability },
}
```
