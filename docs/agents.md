# Runact Agents

> **Status:** Design document. This document describes how to build agent systems on top of Runact's actor primitives.

## Overview

Agents in Runact are runtime entities that interact with the system through a controlled, auditable interface. An agent is NOT the LLM itself — it is an actor that wraps an LLM adapter and executes operations through the standard message system.

This pattern applies to any web application that needs to manage AI agent lifecycles: chatbots, autonomous coding assistants, tool-using agents, or multi-agent orchestration systems.

## Architecture

```text
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

## Core Principle

An agent is implemented as a supervised actor.

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

impl Actor for AgentProcess {
    type Message = AgentMessage;

    fn handle(&mut self, message: AgentMessage, context: &mut ActorContext) -> Result<(), ActorError> {
        match message {
            AgentMessage::ExecuteTask(task) => {
                self.execute_task(task, context);
            }
            AgentMessage::ReceiveObservation(obs) => {
                self.process_observation(obs, context);
            }
            AgentMessage::RequestApproval(tx_id) => {
                self.handle_approval(tx_id, context);
            }
        }
        Ok(())
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

Agent capabilities are managed through `Capability<H: ResourceHandle>` — the same resource capability system described in [Security](security.md).

## Status

This feature is not yet implemented in v1.0.0. The runtime provides the foundational primitives (actors, supervisors, compute, timers, capabilities) that an agent system would build upon, but the agent-specific abstractions are planned for a future version.
