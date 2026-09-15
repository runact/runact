# Runact Transactions

## Overview

Transactions are a first-class concept in Runact. They ensure atomic, consistent, and auditable state mutations. This is critical for AI agent integration where changes must be previewed, approved, and tracked.

## Purpose

- Atomic multi-step operations
- Preview changes before applying
- Rollback on failure
- Audit trail for all mutations
- Conflict detection
- Safe AI agent integration

## Transaction Lifecycle

```
Requester
  │
  │ proposed changes
  ▼
Transaction
  │
  ├── validate
  ├── preview
  ├── review
  └── audit
        │
    ┌───┴───┐
    ▼       ▼
 Commit   Rollback
```

## Transaction Structure

```rust
pub struct Transaction {
    pub id: TransactionId,
    pub requester_id: ProcessId,
    pub operations: Vec<Operation>,
    pub status: TransactionStatus,
    pub created_at: Timestamp,
    pub approved_at: Option<Timestamp>,
    pub committed_at: Option<Timestamp>,
    pub metadata: TransactionMetadata,
}
```

### Operation

A single atomic operation within a transaction.

```rust
pub struct Operation {
    pub operation_type: OperationType,
    pub target: Target,
    pub payload: Vec<u8>,
}

pub enum OperationType {
    Create,
    Read,
    Update,
    Delete,
}

pub struct Target {
    pub resource_type: String,
    pub resource_id: String,
}
```

### TransactionStatus

```rust
pub enum TransactionStatus {
    Pending,
    Validating,
    PreviewReady,
    AwaitingApproval,
    Approved,
    Committing,
    Committed,
    RollingBack,
    RolledBack,
    Failed { reason: String },
}
```

### TransactionMetadata

```rust
pub struct TransactionMetadata {
    pub description: Option<String>,
    pub correlation_id: Option<CorrelationId>,
    pub parent_transaction: Option<TransactionId>,
    pub tags: Vec<String>,
}
```

## Transaction Operations

### Create

```rust
impl Transaction {
    pub fn new(requester_id: ProcessId) -> Self {
        Self {
            id: TransactionId::new(),
            requester_id,
            operations: Vec::new(),
            status: TransactionStatus::Pending,
            created_at: Timestamp::now(),
            approved_at: None,
            committed_at: None,
            metadata: TransactionMetadata::default(),
        }
    }
}
```

### Add Operation

```rust
impl Transaction {
    pub fn add_operation(&mut self, operation: Operation) -> Result<(), TransactionError> {
        if self.status != TransactionStatus::Pending {
            return Err(TransactionError::InvalidState(self.status));
        }

        self.operations.push(operation);
        Ok(())
    }
}
```

### Validate

Check that all operations are valid before previewing.

```rust
impl Transaction {
    pub async fn validate(&self, runtime: &Runtime) -> Result<(), ValidationError> {
        for operation in &self.operations {
            // Check target exists for update/delete
            match operation.operation_type {
                OperationType::Update | OperationType::Delete => {
                    if !runtime.resource_exists(&operation.target).await? {
                        return Err(ValidationError::ResourceNotFound {
                            target: operation.target.clone(),
                        });
                    }
                }
                _ => {}
            }

            // Check payload validity
            runtime.validate_payload(&operation.operation_type, &operation.payload)?;
        }

        Ok(())
    }
}
```

### Preview

Generate a diff showing proposed changes.

```rust
impl Transaction {
    pub async fn preview(&self, runtime: &Runtime) -> Result<Vec<Diff>, PreviewError> {
        let mut diffs = Vec::new();

        for operation in &self.operations {
            let diff = runtime.preview_operation(operation).await?;
            diffs.push(diff);
        }

        Ok(diffs)
    }
}
```

### Approve

Mark transaction as approved (by user or automatic policy).

```rust
impl Transaction {
    pub fn approve(&mut self) -> Result<(), TransactionError> {
        if self.status != TransactionStatus::AwaitingApproval {
            return Err(TransactionError::InvalidState(self.status));
        }

        self.status = TransactionStatus::Approved;
        self.approved_at = Some(Timestamp::now());
        Ok(())
    }
}
```

### Commit

Apply all operations atomically.

```rust
impl Transaction {
    pub async fn commit(&mut self, runtime: &Runtime) -> Result<(), CommitError> {
        if self.status != TransactionStatus::Approved {
            return Err(CommitError::NotApproved);
        }

        self.status = TransactionStatus::Committing;

        // Apply operations in order
        for operation in &self.operations {
            runtime.apply_operation(operation).await?;
        }

        self.status = TransactionStatus::Committed;
        self.committed_at = Some(Timestamp::now());

        // Emit event
        runtime.emit_event(Event::TransactionCommitted {
            transaction_id: self.id,
        });

        Ok(())
    }
}
```

### Rollback

Revert all changes if commit fails or is rejected.

```rust
impl Transaction {
    pub async fn rollback(&mut self, runtime: &Runtime) -> Result<(), RollbackError> {
        self.status = TransactionStatus::RollingBack;

        // Reverse all applied operations
        for operation in self.operations.iter().rev() {
            runtime.reverse_operation(operation).await?;
        }

        self.status = TransactionStatus::RolledBack;

        // Emit event
        runtime.emit_event(Event::TransactionRolledBack {
            transaction_id: self.id,
        });

        Ok(())
    }
}
```

## Conflict Detection

### Optimistic Concurrency

Transactions use version vectors to detect conflicts.

```rust
pub struct ResourceVersion {
    pub resource_type: String,
    pub resource_id: String,
    pub version: u64,
}

impl Transaction {
    pub fn with_versions(mut self, versions: Vec<ResourceVersion>) -> Self {
        self.metadata.resource_versions = versions;
        self
    }
}
```

### Conflict Detection

```rust
impl Transaction {
    pub async fn detect_conflicts(&self, runtime: &Runtime) -> Result<Vec<Conflict>, ConflictError> {
        let mut conflicts = Vec::new();

        for operation in &self.operations {
            let current_version = runtime.get_resource_version(&operation.target).await?;
            let expected_version = self.metadata.resource_versions
                .iter()
                .find(|v| v.resource_type == operation.target.resource_type
                    && v.resource_id == operation.target.resource_id)
                .map(|v| v.version);

            if let Some(expected) = expected_version {
                if current_version != expected {
                    conflicts.push(Conflict {
                        target: operation.target.clone(),
                        expected_version: expected,
                        actual_version: current_version,
                    });
                }
            }
        }

        Ok(conflicts)
    }
}
```

## Multi-Resource Transactions

Transactions can span multiple resources atomically.

```rust
impl Transaction {
    pub fn multi_resource(operations: Vec<Operation>) -> Self {
        let mut tx = Self::new(ProcessId::new(0));
        tx.operations = operations;
        tx
    }
}
```

## Audit Trail

Every transaction is recorded for auditing.

```rust
pub struct AuditEntry {
    pub transaction_id: TransactionId,
    pub requester_id: ProcessId,
    pub action: AuditAction,
    pub timestamp: Timestamp,
    pub details: String,
}

pub enum AuditAction {
    Created,
    Validated,
    Previewed,
    Approved,
    Committed,
    RolledBack,
    Failed { reason: String },
}
```

### Audit Storage

```rust
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

impl AuditLog {
    pub fn record(&mut self, entry: AuditEntry);
    pub fn query_by_transaction(&self, id: TransactionId) -> Vec<&AuditEntry>;
    pub fn query_by_requester(&self, id: ProcessId) -> Vec<&AuditEntry>;
    pub fn query_by_time(&self, start: Timestamp, end: Timestamp) -> Vec<&AuditEntry>;
}
```

## Approval Flow

### Automatic Approval

```rust
pub struct ApprovalPolicy {
    pub rules: Vec<ApprovalRule>,
}

pub enum ApprovalRule {
    AlwaysApprove,
    AlwaysDeny,
    ApproveIf { condition: ApprovalCondition },
    RequireApproval { approver: ApprovalSource },
}
```

### User Approval

For dangerous operations:

```rust
impl Transaction {
    pub async fn request_approval(&mut self, runtime: &Runtime) -> Result<(), ApprovalError> {
        self.status = TransactionStatus::AwaitingApproval;

        // Generate preview
        let preview = self.preview(runtime).await?;

        // Show to user
        runtime.show_transaction_preview(self.id, preview).await?;

        // Wait for user decision
        let decision = runtime.wait_for_approval(self.id).await?;

        match decision {
            ApprovalDecision::Approve => self.approve(),
            ApprovalDecision::Deny => {
                self.status = TransactionStatus::Failed {
                    reason: "User denied".to_string(),
                };
                Err(ApprovalError::Denied)
            }
        }
    }
}
```

## Agent Integration

### Agent Transaction Flow

```rust
impl AgentProcess {
    pub async fn propose_operation(
        &self,
        runtime: &Runtime,
        operation: Operation,
    ) -> Result<TransactionId, AgentError> {
        let mut tx = Transaction::new(self.id);
        tx.add_operation(operation)?;

        // Validate
        tx.validate(runtime).await?;

        // Create preview
        let preview = tx.preview(runtime).await?;

        // Request approval
        tx.request_approval(runtime).await?;

        // Commit
        tx.commit(runtime).await?;

        Ok(tx.id)
    }
}
```

### Agent Capabilities

Agents must have capabilities to create transactions:

```rust
pub struct AgentCapabilities {
    pub can_propose_operations: bool,
    pub can_commit_transactions: bool,
    pub max_transaction_size: usize,
}
```

## Error Handling

### TransactionError

```rust
pub enum TransactionError {
    InvalidState(TransactionStatus),
    OperationConflict { target: Target },
    ValidationFailed(ValidationError),
    CommitFailed(CommitError),
    RollbackFailed(RollbackError),
}
```

### Recovery

If a transaction fails mid-commit:

1. Attempt rollback
2. If rollback fails, mark resource as corrupted
3. Notify supervisor
4. Log full transaction state for debugging

## Observability

### Transaction Metrics

```rust
pub struct TransactionMetrics {
    pub transactions_created: u64,
    pub transactions_committed: u64,
    pub transactions_rolled_back: u64,
    pub transactions_failed: u64,
    pub average_commit_time: Duration,
    pub conflict_count: u64,
}
```

### Tracing

Every transaction operation is traced:

```rust
#[instrument(skip(runtime), fields(transaction_id = %self.id))]
async fn commit(&mut self, runtime: &Runtime) -> Result<(), CommitError> {
    // ...
}
```
