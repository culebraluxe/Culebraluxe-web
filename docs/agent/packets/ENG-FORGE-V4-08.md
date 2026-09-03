# ENG-FORGE-V4-08 — Execution Contract Gate

## Goal

Make Forge prove that a Smith launch has a complete, internally consistent execution contract before any builder process starts.

This is the first controlled **Forge-builds-Forge** story. The Architect packet is written by Chris + ChatGPT; implementation must be performed by Forge in its isolated worker flow.

## Architect brief

Add one explicit, testable execution-contract validation seam before Smith is launched.

The gate must validate the fully merged story packet plus the resolved runtime assignment. It must fail closed with concrete reasons; it must never silently invent missing values or fall back to a different player/field/profile.

For a Smith launch, require all of the following:

1. non-empty Architect brief
2. non-empty acceptance criteria
3. non-empty Assay commands / Assay plan
4. explicit execution target on the work envelope (`DEV|PROD|TEST|LOCAL`), with this story continuing to execute against DEV
5. assigned logical profile exists
6. selected adapter/player runtime is ready under the V4-07 readiness gate
7. assigned Field is ready
8. Smith-required capabilities are satisfied by the selected runtime/profile

Reject contradictory states explicitly. Examples include a Smith envelope with a missing brief, an unavailable field, an unready adapter, or a profile that does not satisfy Smith capabilities.

The validation result should be a small provider-neutral contract such as `ok + reasons/code`, reusable by hydration/launch boundaries without embedding DeepSeek/Warp/OpenClaw model names in orchestration logic.

Enforce the gate at the latest safe point **before external Smith execution begins**. Existing V3 defenses remain in place; this story strengthens them rather than replacing them.

Smith remains the only role allowed to retain a commit. Assay remains read-only and owns acceptance. A work item becoming Done must not imply Story Complete.

## Context refs

Read these first and keep the change bounded:

- `agent-runtime/orchestrate-apply.ts` — Ready hydration and followed-lane enqueue behavior
- `agent-runtime/enqueue-lane.ts` — canonical lane envelope construction
- `agent-runtime/lane-policy.ts` and `agent-runtime/lanes.ts` — Smith requirements/capabilities
- `agent-runtime/story-session.ts` and `agent-runtime/git-packet.ts` — merged packet truth
- `agent-runtime/registry.ts` and `agent-runtime/readiness.ts` — profile/adapter readiness boundary
- `agent-runtime/team.ts` — Player/Harness/Field assignment truth
- `agent-runtime/invoker.ts` and `scripts/agent-work.ts` — final pre-execution launch boundary
- existing V3/V4 tests around hydration, routing, readiness and Assay

Do not broaden reconnaissance outside these seams unless a direct dependency requires it.

## Scope

Expected shape, not mandatory filenames:

- one small provider-neutral execution-contract validator under `agent-runtime/`
- the narrowest hydration/enqueue/invoker integration needed to make the gate authoritative
- focused tests for complete and incomplete Smith contracts
- no schema migration
- no UI work unless required to surface an already-existing rejection reason

Do not refactor unrelated Forge code while implementing this story.

## Acceptance criteria

- a fully complete Smith contract passes unchanged
- missing Architect brief rejects Smith before external execution
- missing acceptance criteria rejects Smith before external execution
- missing Assay commands/plan rejects Smith before external execution
- missing/invalid execution target rejects Smith before external execution
- unknown profile rejects Smith
- registered-but-unready adapter rejects Smith
- unavailable Field rejects Smith
- insufficient Smith capabilities reject Smith
- rejection evidence names the failing contract condition; no silent fallback is permitted
- Scout behavior is unchanged
- current V3 Scout → Smith → Assay ordering is unchanged
- Architect is not auto-run by this story
- Assay failure still means Hold/repair, never Story Complete
- only Smith/builder may retain a commit
- no provider is activated
- no Warp swarm/parallel execution is activated
- no database/schema changes
- the existing 42-test Forge seam remains green in addition to the new contract-gate tests

## Test mode

SCOPED

## Assay commands

- pnpm exec tsx --test agent-runtime/execution-contract.test.ts
- pnpm exec tsx --test agent-runtime/readiness.test.ts agent-runtime/gateway/cli-agent-adapter.test.ts agent-runtime/gateway/provider.test.ts agent-runtime/team.test.ts agent-runtime/orchestrate.test.ts agent-runtime/orchestrate-apply.test.ts agent-runtime/repositories.assay.test.ts

## Out of scope

- recursive story invention
- self-modifying the currently running checkout
- automatic Architect execution
- story decomposition
- multiple Smiths
- parallelism or swarm activation
- Warp cloud execution
- OpenCode/Pi integration
- Grok/Meta/OpenAI provider qualification
- changing the Player roster
- broad regression runs
- Neon schema changes

## Self-build boundary

Forge N may create a candidate Forge N+1 only inside its isolated worktree/branch pinned to the approved base. It must not modify, replace, restart, or reconfigure the control plane/runtime process that is currently executing it. The candidate commit is evidence only until Assay passes and Chris/Architect accepts it.

## Builder report back

Report:

1. exact files changed
2. exact pre-Smith gate location
3. every fail-closed condition implemented
4. exact scoped tests run and counts
5. worker commit hash
6. anything the packet requested that was intentionally not implemented, with reason

Do not push or merge.