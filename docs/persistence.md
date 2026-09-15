# Runact Persistence

## Overview

Persistence in Runact ensures that runtime state survives restarts. Critical transactions must have recoverable state. All persisted formats must support migrations.

## What to Persist

### Required

- Runtime metadata
- Process registry
- Extensions
- Configuration
- Audit information

### Not Persisted (Initially)

- Process state (except agents)
- Mailbox contents
- In-flight messages
- Temporary data

## Storage Architecture

```
┌─────────────────────────────────────┐
│           Storage API               │
│    (trait-based abstraction)        │
└──────────────┬──────────────────────┘
               │
    ┌──────────┼──────────┐
    ▼          ▼          ▼
 SQLite    SledDB    Filesystem
```

### Storage Trait

```rust
#[async_trait]
pub trait Storage: Send + Sync {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;
    async fn put(&self, key: &[u8], value: &[u8]) -> Result<(), StorageError>;
    async fn delete(&self, key: &[u8]) -> Result<(), StorageError>;
    async fn exists(&self, key: &[u8]) -> Result<bool, StorageError>;
    async fn list(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>, StorageError>;

    async fn transaction<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut Transaction) -> R + Send;

    fn snapshot(&self) -> Box<dyn Storage>;
}
```

### Storage Error

```rust
pub enum StorageError {
    NotFound(Vec<u8>),
    AlreadyExists(Vec<u8>),
    SerializationError(String),
    IoError(std::io::Error),
    TransactionError(String),
}
```

## Schema Versioning

### Versioned Schemas

Every persisted format has a version.

```rust
pub struct VersionedSchema<T> {
    pub version: SchemaVersion,
    pub data: T,
}

pub struct SchemaVersion {
    pub major: u16,
    pub minor: u16,
}
```

### Schema Registry

```rust
pub struct SchemaRegistry {
    schemas: HashMap<String, Box<dyn Schema>>,
}

pub trait Schema: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> SchemaVersion;
    fn migrate_from(&self, from: &dyn Schema, data: Vec<u8>) -> Result<Vec<u8>, MigrationError>;
}
```

## Migrations

### Migration Chain

```
v1 → v2 → v3 → v4 (current)
```

### Migration Implementation

```rust
pub struct MigrationChain {
    migrations: Vec<Box<dyn Migration>>,
}

pub trait Migration: Send + Sync {
    fn from_version(&self) -> SchemaVersion;
    fn to_version(&self) -> SchemaVersion;
    fn migrate(&self, data: Vec<u8>) -> Result<Vec<u8>, MigrationError>;
}

impl MigrationChain {
    pub fn migrate(&self, data: Vec<u8>, from: SchemaVersion) -> Result<Vec<u8>, MigrationError> {
        let mut current = data;
        let mut current_version = from;

        for migration in &self.migrations {
            if migration.from_version() == current_version {
                current = migration.migrate(current)?;
                current_version = migration.to_version();
            }
        }

        Ok(current)
    }
}
```

### Example Migration

```rust
pub struct V1ToV2Migration;

impl Migration for V1ToV2Migration {
    fn from_version(&self) -> SchemaVersion {
        SchemaVersion { major: 1, minor: 0 }
    }

    fn to_version(&self) -> SchemaVersion {
        SchemaVersion { major: 2, minor: 0 }
    }

    fn migrate(&self, data: Vec<u8>) -> Result<Vec<u8>, MigrationError> {
        let v1: RuntimeV1 = bincode::deserialize(&data)?;
        let v2 = RuntimeV2 {
            id: v1.id,
            name: v1.name,
            created_at: v1.created_at,
            // New field with default
            configuration: RuntimeConfig::default(),
        };
        Ok(bincode::serialize(&v2)?)
    }
}
```

## Persisted Types

### Runtime

```rust
#[derive(Serialize, Deserialize)]
pub struct RuntimePersisted {
    pub id: RuntimeId,
    pub name: String,
    pub processes: Vec<ProcessId>,
    pub extensions: Vec<ExtensionId>,
    pub configuration: RuntimeConfig,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub schema_version: SchemaVersion,
}
```

### Process

```rust
#[derive(Serialize, Deserialize)]
pub struct ProcessPersisted {
    pub id: ProcessId,
    pub name: Option<String>,
    pub parent: Option<ProcessId>,
    pub state: ProcessState,
    pub created_at: Timestamp,
    pub schema_version: SchemaVersion,
}
```

### Buffer

```rust
#[derive(Serialize, Deserialize)]
pub struct BufferPersisted {
    pub id: BufferId,
    pub project_id: ProjectId,
    pub name: String,
    pub path: Option<PathBuf>,
    pub language: Option<String>,
    pub encoding: Encoding,
    pub content: String,  // Serialized rope
    pub is_dirty: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub schema_version: SchemaVersion,
}
```

### Agent

```rust
#[derive(Serialize, Deserialize)]
pub struct AgentPersisted {
    pub id: AgentId,
    pub name: String,
    pub capabilities: Vec<Capability>,
    pub context: AgentContext,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub schema_version: SchemaVersion,
}
```

### Audit Log

```rust
#[derive(Serialize, Deserialize)]
pub struct AuditEntryPersisted {
    pub entry_id: MessageId,
    pub transaction_id: TransactionId,
    pub agent_id: Option<AgentId>,
    pub action: AuditAction,
    pub timestamp: Timestamp,
    pub details: String,
    pub schema_version: SchemaVersion,
}
```

## Recovery

### Recovery Process

1. Load persisted state
2. Validate schema version
3. Run migrations if needed
4. Reconstruct runtime state
5. Verify consistency

```rust
impl Runtime {
    pub async fn recover(&mut self) -> Result<(), RecoveryError> {
        // Load runtime
        let runtime_data = self.storage.get(b"runtime")?;
        let runtime: RuntimePersisted = if let Some(data) = runtime_data {
            let migrated = self.schema_registry.migrate("runtime", data)?;
            bincode::deserialize(&migrated)?
        } else {
            RuntimePersisted::new()
        };

        // Load processes
        for process_id in &runtime.processes {
            let process_data = self.storage.get(&process_id.to_bytes())?;
            if let Some(data) = process_data {
                let migrated = self.schema_registry.migrate("process", data)?;
                let process: ProcessPersisted = bincode::deserialize(&migrated)?;
                self.restore_process(process)?;
            }
        }

        // Load extensions
        for extension_id in &runtime.extensions {
            let extension_data = self.storage.get(&extension_id.to_bytes())?;
            if let Some(data) = extension_data {
                let migrated = self.schema_registry.migrate("extension", data)?;
                let extension: ExtensionPersisted = bincode::deserialize(&migrated)?;
                self.restore_extension(extension)?;
            }
        }

        // Load audit log
        self.load_audit_log()?;

        Ok(())
    }
}
```

### Incomplete Transaction Recovery

```rust
impl Runtime {
    async fn recover_incomplete_transactions(&mut self) -> Result<(), RecoveryError> {
        let incomplete = self.storage.list(b"transaction:pending:")?;

        for tx_data in incomplete {
            let tx: TransactionPersisted = bincode::deserialize(&tx_data)?;

            match tx.status {
                TransactionStatus::Committing => {
                    // Transaction was interrupted during commit
                    // Rollback to be safe
                    tracing::warn!("Rolling back incomplete transaction: {}", tx.id);
                    self.rollback_transaction(tx.id).await?;
                }
                TransactionStatus::Pending => {
                    // Transaction was never started
                    // Discard
                    tracing::warn!("Discarding pending transaction: {}", tx.id);
                    self.discard_transaction(tx.id).await?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
```

## Consistency

### Write-Ahead Log

Critical operations use write-ahead logging.

```rust
pub struct WriteAheadLog {
    storage: Box<dyn Storage>,
}

impl WriteAheadLog {
    pub async fn begin(&mut self, operation_id: OperationId) -> Result<(), WALError> {
        self.storage.put(
            &format!("wal:{}", operation_id).as_bytes(),
            b"pending",
        ).await
    }

    pub async fn commit(&mut self, operation_id: OperationId) -> Result<(), WALError> {
        self.storage.delete(&format!("wal:{}", operation_id).as_bytes()).await
    }

    pub async fn rollback(&mut self, operation_id: OperationId) -> Result<(), WALError> {
        self.storage.delete(&format!("wal:{}", operation_id).as_bytes()).await
    }

    pub async fn recover(&self) -> Result<Vec<OperationId>, WALError> {
        let pending = self.storage.list(b"wal:")?;
        Ok(pending.into_iter().map(|k| OperationId::from_bytes(&k)).collect())
    }
}
```

### Checkpointing

Periodic snapshots for fast recovery.

```rust
pub struct Checkpointer {
    storage: Box<dyn Storage>,
    interval: Duration,
}

impl Checkpointer {
    pub async fn checkpoint(&self, state: &RuntimeState) -> Result<(), CheckpointError> {
        let snapshot = state.snapshot()?;
        let data = bincode::serialize(&snapshot)?;

        self.storage.put(b"checkpoint:latest", &data).await?;

        Ok(())
    }

    pub async fn restore(&self) -> Result<Option<RuntimeState>, CheckpointError> {
        let data = self.storage.get(b"checkpoint:latest")?;

        match data {
            Some(bytes) => {
                let snapshot: RuntimeSnapshot = bincode::deserialize(&bytes)?;
                Ok(Some(snapshot.restore()))
            }
            None => Ok(None),
        }
    }
}
```

## Export/Import

### Export

```rust
impl Runtime {
    pub async fn export(&self, path: &Path) -> Result<(), ExportError> {
        let export_data = ExportData {
            workspace: self.workspace.persist()?,
            projects: self.projects.iter().map(|p| p.persist()).collect(),
            agents: self.agents.iter().map(|a| a.persist()).collect(),
            audit_log: self.audit_log.export()?,
            schema_version: CURRENT_SCHEMA_VERSION,
        };

        let data = bincode::serialize(&export_data)?;
        tokio::fs::write(path, data).await?;

        Ok(())
    }
}
```

### Import

```rust
impl Runtime {
    pub async fn import(&mut self, path: &Path) -> Result<(), ImportError> {
        let data = tokio::fs::read(path).await?;
        let export_data: ExportData = bincode::deserialize(&data)?;

        // Migrate if needed
        let migrated = self.schema_registry.migrate_export(export_data)?;

        // Import workspace
        self.workspace = Workspace::from_persisted(migrated.workspace)?;

        // Import projects
        for project_data in migrated.projects {
            let project = Project::from_persisted(project_data)?;
            self.projects.insert(project.id, project);
        }

        // Import agents
        for agent_data in migrated.agents {
            let agent = Agent::from_persisted(agent_data)?;
            self.agents.insert(agent.id, agent);
        }

        // Import audit log
        self.audit_log.import(migrated.audit_log)?;

        Ok(())
    }
}
```

## Observability

### Persistence Metrics

```rust
pub struct PersistenceMetrics {
    pub reads: u64,
    pub writes: u64,
    pub deletes: u64,
    pub transactions: u64,
    pub migrations: u64,
    pub checkpoints: u64,
    pub recovery_time: Duration,
}
```

### Storage Health

```rust
pub struct StorageHealth {
    pub total_keys: usize,
    pub total_bytes: usize,
    pub fragmentation: f64,
    pub last_checkpoint: Option<Timestamp>,
    pub pending_wal_entries: usize,
}
```
