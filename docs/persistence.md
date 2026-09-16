# Runact Persistence

> **Status:** Planned feature. Not yet implemented in v1.0.0. This document describes the intended persistence system for the PaperOS runtime built on top of Runact.

## Overview

Persistence ensures that runtime state survives restarts. Critical transactions must have recoverable state. All persisted formats must support migrations.

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

```text
┌─────────────────────────────────────┐
│           Storage API               │
│    (trait-based abstraction)        │
└──────────────┬──────────────────────┘
               │
    ┌──────────┼──────────┐
    ▼          ▼          ▼
 SQLite    SledDB    Filesystem
```

## Status

This feature is not yet implemented in v1.0.0. The runtime provides actors, supervisors, and compute primitives that a persistence layer would build upon, but persistence itself is planned for a future version.
