# Current Machine — Forge SDLC (rewritten 2026-09-13)

This file used to describe the CRM-07 WhatsApp intake tranche and had been stale since
V10; anyone reading it to orient aimed at the wrong story. It now describes the live
machine. Story packets live in `docs/agent/packets/`, durable decisions in
`docs/agent/MEMORY.md`.

## What runs

1. Engine: `workflow_app/definitions/FORGE_SDLC-v6.xml`, driven by
   `workflow_app/forge/forge-executor.ts`.
2. Roles are phase agents in `workflow_app/forge/agents/role-agents.ts`. They `collect()`
   evidence; the parent gate (`forge-phase-agent.ts`) stays the decider.
3. A role runs in its own worktree under `~/Documents/Culebraluxe-worktrees/<story>-<id>-e0`
   via the OpenCode harness (`agent-runtime/opencode/`).

## The one doctrine that matters

The failure mode is the CHANNEL, not the parser. A decision must never live only in a
model's chat reply.

1. Decisions travel in rows: `forge_role_contract` (migration 170) for the decision,
   `forge_role_plan_chunk` / `forge_role_assignment` (171) for the plan.
2. Reply markers (`LEAD_ROUTING:`, `FORGE_ARCHITECT_HANDOFF:`) remain as the FALLBACK for
   rows that were never written. Fields win; text is the fallback.
3. `workflow_app/forge/lead-proposal-resolve.ts` is the ONE seat for the Lead decision:
   collect and the runner both resolve through it, so a refusal can never be re-reviewed
   and accepted by a second evaluator.

## Measured facts (do not re-derive these)

1. One role's model call is about 39 seconds; role wall clock measured 52 seconds on
   2026-09-13. Harness startup is 0.30s and config load 0.55s, so startup is not the cost
   — inference is.
2. The `rtk` shim used to fork-bomb every tool call (4,627 processes, load 34, about 80s
   blocked per call, every parent asleep on its child). Fixed in `forge-tool-seams.ts`:
   the shim strips its own directory from PATH before exec, with a regression test that
   fails if that is reintroduced.
3. Forge runs execute against PROD only. See `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md`.

## Open, in priority order

1. Lead cutover: wired (fields first, one seat, bench cap no longer silent) and tested
   DB-free; still needs a live run to confirm a fields-only story routes end to end.
2. Smith authorization: the serial lane treats an unreadable diff as a measurement gap
   rather than a scope miss. Deliberate today; changing it is the captain's call.
3. Completion atomicity: durable evidence is merged BEFORE the engine CAS
   (`forge-engine-runtime.ts`), so a losing worker's evidence is already committed.
4. `scripts/forge-handoff.mjs` splits list flags on commas.
