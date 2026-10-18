# Agent OS Executive Layer

Phase 1 adds an executive reasoning layer **above** the existing Forge autonomous SDLC. It does not replace Forge, create a second workflow engine, or duplicate Story Board state.

## Roles

| Role | Primary question | Card |
| --- | --- | --- |
| Chief of Staff | What matters now and what genuinely needs Chris? | `CHIEF-OF-STAFF.md` |
| Advisor | What should Chris do and why? | `ADVISOR.md` |
| Principal Architect | How should the system be shaped? | `PRINCIPAL-ARCHITECT.md` |
| Adversary | Why might the favored answer be wrong? | `ADVERSARY.md` |
| Expert Council | How do we resolve a consequential multi-domain choice? | `COUNCIL.md` |

## Relationship to Forge

```text
Chris
  |
Executive layer
  |
accepted intent / architecture / owner decision
  |
Story Board + Forge
  |
Scout -> Architect -> Lead -> Smith -> Inspector/Assay -> DEV_OPS
```

The existing Forge Architect is a per-story runtime service. The Principal Architect is the cross-project architecture authority. Keep both roles distinct.

## Start here

Read:

1. `../AGENT-OS-PHASE-1.md`
2. the relevant executive role card
3. `../ROLE-REGISTRY.md`
4. `../ESCALATION-QUEUES.md` when work is blocked or needs a decision

For executable software work, continue through the existing Forge/Story Board path rather than inventing an executive-only execution path.