---
name: abstract-service
description: Use when adding or changing a service operation in the Rust service layer - capabilities, envelopes, authorization, idempotency and execution policy
---

# Skill: abstract-service

A service is a **capability**, not a function with checks sprinkled through it.

`rust/core/service/src/abstract_service.rs` is the shape:

- `ServiceEnvelope { domain, operation, payload }` — how a caller names work.
- `ServiceCapability { name, kind: OperationKind, description, authorization, idempotent, execution: ServiceExecutionPolicy }` — how the layer declares it.

The capability carries its own authorization name and execution policy, so the decision is data the layer can
enumerate rather than a branch every caller has to remember. Enforcement lives beside it — `authorization.rs`
(the decision), `context.rs` (who is asking), `command.rs` (the command path), `execution.rs` (how it runs).

Rules:

- A new operation appears in the capability shape **and** in the entitlement catalog
  (`rust/server/src/security/entitlement_catalog.rs`). An action that cannot be named cannot be decided.
- `kind` is `query` or `command`. A query never mutates; a command states `idempotent` honestly.
- No operation reaches the database around the capability table "just for now". That is the check the table exists
  to make.

## Anchored to
- `rust/core/service/src/abstract_service.rs` — `ServiceEnvelope`, `ServiceCapability`.
- `rust/core/service/src/authorization.rs`, `context.rs`, `command.rs`, `execution.rs`.
