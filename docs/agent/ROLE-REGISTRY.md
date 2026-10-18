# Agent OS Role Registry

**Status:** Active Phase 1 role/authority map  
**Rule:** this registry maps responsibility; it does not replace Forge's XML service bindings or OpenCode permission enforcement.

## Layer 0 — Principal

| Role | Purpose | Can write app code | Can publish/deploy | Escalates to |
| --- | --- | ---: | ---: | --- |
| Chris / CTO / Product Owner | Product intent, priority, owner-only and irreversible decisions | Yes by owner choice | Yes / authorizes | — |

## Layer 1 — Executive intelligence

| Role | Purpose | Can write app code | Can publish/deploy | Escalates to |
| --- | --- | ---: | ---: | --- |
| Chief of Staff | Continuity, priorities, routing, owner-attention filter | No | No | Chris / relevant executive role |
| Advisor | Strategic judgment, trade-offs, recommendation | No | No | Council / Chris |
| Principal Architect | Cross-project architectural policy and boundary resolution | No by default | No | Council / Chris |
| Expert Council | Multi-domain decision escalation | No | No | Chris when owner authority is required |

These roles sit above Forge. In Phase 1 they do not create competing workflow nodes or queue tables.

## Layer 2 — Forge autonomous SDLC

The runtime roles below already exist as Rust services and are bound to workflow nodes by `FORGE_SDLC-v6.xml`. Their execution authority is enforced through Forge/OpenCode; this table is explanatory, not the source of runtime permissions.

| Runtime role | Service | Responsibility | Workspace authority | Publication/release authority | Normal handoff |
| --- | --- | --- | --- | --- | --- |
| Scout | `forge.scout` | Repository facts, diagnosis, discovery | Read-only | None | Architect |
| Architect | `forge.architect` | Per-story frozen implementation contract | Read-only | None | Lead |
| Lead | `forge.lead` | Routing, execution shape, assembly | Read-only except explicit solo-implement phase | None | Smith/Assay/QA path |
| Smith | `forge.smith` | Implementation and developer-side proof | Implement | None | Lead/QA |
| Inspector | `forge.inspector` | Independent source review | QA read-only; **no Git** | None | QA verdict path |
| Assay | `forge.assay` | Approved deterministic verification | QA read-only; **no Git** | None | QA verdict path |
| DEV_OPS | `forge.devops` | Release-state analysis and DEV_OPS role work | Read-only model profile; release actions stay Forge-owned code | Canonical owner when authorized | release/production result |

### Non-negotiable QA boundary

`Inspector` and `Assay` are independent QA. They may measure and report. They may not edit source/tests, use Git, repair findings, publish, migrate, deploy, or take release authority. `docs/agent/FORGE-QA-BOUNDARY.md` is the binding policy.

## Layer 3 — Micro-subagents / specialists

Current OpenCode micro-subagents already include:

| Agent | Purpose | Authority |
| --- | --- | --- |
| `forge-explore` | Search, dependency tracing, call-site discovery | Read-only |
| `forge-reviewer` | Defect/architecture/missing-test review | Read-only |
| `forge-test-analyst` | Locate tests and coverage gaps | Read-only |

They are bounded children, not independent workflow owners. Existing Forge arming/depth/child-count rules remain authoritative.

## Executive vs Forge Architect

This distinction is mandatory:

```text
Principal Architect
  -> cross-system policy, long-lived boundaries, architecture disputes

Forge Architect
  -> one story, one frozen implementation contract inside FORGE_SDLC
```

The Principal Architect may create or amend durable architecture policy. A normal story still goes through the Forge Architect node when the XML says it does.

## Advisor vs Lead

```text
Advisor
  -> should we do it? what is the best strategic choice?

Lead
  -> given accepted intent/architecture, what execution shape delivers it?
```

Lead does not silently convert a strategic/product ambiguity into an implementation decision.

## Chief of Staff vs Workflow engine

```text
Chief of Staff
  -> what matters, what needs routing, what requires Chris

Workflow/XML
  -> what executable state/node runs next
```

The Chief of Staff never becomes a shadow scheduler.

## Autonomous role envelope

Every autonomous role must have:

1. objective;
2. allowed actions;
3. forbidden actions;
4. escalation conditions;
5. required output/evidence;
6. handoff target;
7. runtime authority enforcement where side effects exist.

A prose card can narrow behavior; it cannot grant authority the runtime denies.

## Escalation ladder

Use the lowest authority capable of resolving the issue:

```text
role
  -> Lead (execution/ownership)
  -> Principal Architect or Advisor (architecture/strategy)
  -> Council (multi-domain consequential disagreement)
  -> Chris (product intent / owner-only authority)
```

Security, data-integrity, production, destructive, or missing-credential conditions may jump directly to the canonical owner required by policy.

## Adding a specialist later

Do not add a permanent specialist because a topic appeared once. Add one when repeated work demonstrates a stable domain, distinct authority envelope, and measurable value.

Phase 2 may add Security, Database, Frontend, Backend, Integration, or other specialist profiles. Until then, existing Forge skills/services and Council invitees cover those domains without creating another permanent role table.