# ADR Template

Use this template for all Architectural Decision Records.

---

# ADR [NUMBER]: [TITLE]

## Status

[Proposed | Accepted | Deprecated | Superseded]

## Date

[YYYY-MM-DD]

## Context

[What is the issue that motivates this decision? What forces are at play?]

## Decision

[What is the change being proposed or decided?]

## Consequences

### Positive

- [Benefit 1]
- [Benefit 2]

### Negative

- [Drawback 1]
- [Drawback 2]

### Risks

- [Risk 1]
- [Risk 2]

## Alternatives Considered

### [Alternative 1]

[Description]

**Pros:**
- [Pro 1]
- [Pro 2]

**Cons:**
- [Con 1]
- [Con 2]

**Reason for rejection:** [Why this was not chosen]

### [Alternative 2]

[Description]

**Pros:**
- [Pro 1]
- [Pro 2]

**Cons:**
- [Con 1]
- [Con 2]

**Reason for rejection:** [Why this was not chosen]

## References

- [Link to relevant documentation]
- [Link to related ADRs]

---

## Example ADR

# ADR 0001: Use Tokio as Initial Runtime Backend

## Status

Accepted

## Date

2025-01-15

## Context

Runact needs an asynchronous runtime for process execution. We need to choose an initial backend that:

- Supports lightweight concurrent processes
- Provides async/await
- Has mature ecosystem
- Can be replaced later

Forces:

- Tokio is the de facto standard for Rust async
- Async-std exists but has smaller ecosystem
- Custom scheduler would be complex and premature

## Decision

Use Tokio as the initial runtime backend, with a Quartz-level abstraction that can be replaced later.

## Consequences

### Positive

- Mature, battle-tested runtime
- Large ecosystem
- Good performance
- Easy to hire for

### Negative

- Tokio types may leak into public APIs (must prevent)
- May not be optimal for actor workloads
- Adds dependency

### Risks

- Tokio abstraction leakage
- Tokio-specific patterns creeping in

## Alternatives Considered

### async-std

**Pros:**
- More "standard library" feel
- Good performance

**Cons:**
- Smaller ecosystem
- Less mature

**Reason for rejection:** Smaller ecosystem makes development harder.

### Custom Scheduler

**Pros:**
- Perfect fit for actor model
- No external dependency

**Cons:**
- Extremely complex
- Premature optimization
- Would take months

**Reason for rejection:** No concrete requirement yet. Can always build later.

## References

- [Tokio documentation](https://tokio.rs/)
- [Runact vision](../vision.md)
