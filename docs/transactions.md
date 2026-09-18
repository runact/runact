# Runact Transactions

> **Status:** Design document. This document describes how to build transaction systems on top of Runact's actor messaging primitives.

## Overview

Transactions ensure atomic, consistent, and auditable state mutations. This is critical for systems where changes must be previewed, approved, and tracked.

This pattern applies to any web application that needs audit trails: payment processing, multi-step form submissions, workflow orchestration, or AI agent tool execution with human-in-the-loop approval.

## Purpose

- Atomic multi-step operations
- Preview changes before applying
- Rollback on failure
- Audit trail for all mutations
- Conflict detection
- Safe AI agent integration

## Transaction Lifecycle

```text
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

## Status

This feature is not yet implemented in v1.0.0. The runtime provides the messaging and compute primitives that a transaction system would build upon, but the transaction API itself is planned for a future version.
