# Runact Extensions

> **Status:** Design document. This document describes how to build extension systems on top of Runact's actor and process primitives.

## Overview

The Runact extension pattern allows external components to interact with the runtime through a versioned, capability-controlled protocol. Extensions run as separate processes and communicate through message passing.

This pattern applies to any web application that needs plugin architectures: extensible API gateways, middleware systems, or modular backends where components are developed and deployed independently.

## Architecture

```text
Runact Runtime
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

## Extension Manifest Structure

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
```

## Extension Communication

Extensions communicate through a versioned binary protocol using the `crossbeam-channel` infrastructure already in the runtime. Message envelopes provide the transport.

## Status

This feature is not yet implemented in v1.0.0. The runtime provides the foundational pieces (compute pool, timers, actors, supervision) that extensions would use, but the extension registration, discovery, and protocol negotiation are planned for a future version.
