# Workflow engine + Forge — this lane is done

Scope: the TypeScript workflow kernel and Forge execution path.
Out of scope: CRM HTTP, portal routes, Next.js, GPT’s `rust/server` slices.

## Engine (rust/core/workflow)
Public operations match `workflow_engine/lib/workflow/engine.ts`:

start, signal, claim/release/reassign/complete task, cancel process,
claim/complete/fail/create job, fire/cancel/reschedule timer, run due jobs,
reclaim stale jobs (global + per instance), requeue failed job,
get instance/token/task/job, instance details, history, list instances,
tasks for user.

Host: `cargo run -p workflow --bin workflow -- start --definition …`
Store: `NeonStore` on the existing Neon schema. No new tables.

## Forge (rust/forge)
XML FORGE_SDLC, drive, OpenCode harness, QA, release, holds, worktree,
self-heal, agent_work claim, decisions, scope, ready-gate, serial Smith
doors, verification anchors, migration ledger guard.

Host: `pnpm forge:engine` → `cargo run -p forge --bin forge`
`scripts/forge-engine-worker.ts` is a shim only.

## What may still exist in TypeScript
Next.js actions and operator UI that *call* the old files. Those are
adapters, not a second engine. Do not add Forge behavior there.
