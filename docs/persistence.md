# Runact Persistence

> **Status:** Design document. This document describes how to build persistence layers on top of Runact's actor and compute primitives.

## Overview

Persistence ensures that runtime state survives restarts. Critical transactions must have recoverable state. All persisted formats must support migrations.

This pattern applies to any web application that needs durable state: session stores, job queues, workflow engines, or database connection pools managed by supervised actors.

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
