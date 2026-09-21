# Current Machine — Forge SDLC (rewritten 2026-09-13)

This file used to describe the CRM-07 WhatsApp intake tranche and had been stale since
V10; anyone reading it to orient aimed at the wrong story. It now describes the live
machine. Story packets live in `docs/agent/packets/`, durable decisions in
`docs/agent/MEMORY.md`.

**Start here instead if you are new to the engine:** `docs/agent/WORKFLOW-ARCHITECTURE.md` — the
layered explainer (simple → advanced → PhD) with the twelve laws, the failure taxonomy, the V7
roadmap, the swarm doctrine, and the diagnostic ladder. Every claim in it is labelled
**[built] / [measured] / [proposed]**, so it can never be mistaken for the target state. This file
is the short form; that file is the depth.

**The Cockpit's purpose, in the captain's words:** `docs/agent/COCKPIT-PURPOSE.md` — the whole
SDLC framing, the three modes of work (waking-hours collaboration / hand over now / loaded but not
firing), why there are TWO engine lists, the invariants that make batching real, and the honest
list of what is not built yet. Read it before changing the board or the engine lanes.

## What runs

1. Engine: `legacy/workflow_app/definitions/FORGE_SDLC-v6.xml`, driven by
   `legacy/workflow_app/forge/forge-executor.ts`.
2. Roles are phase agents in `legacy/workflow_app/forge/agents/role-agents.ts`. They `collect()`
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
3. `legacy/workflow_app/forge/lead-proposal-resolve.ts` is the ONE seat for the Lead decision:
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
4. WARM SESSION (2026-09-13, `ENG-FORGE-WARM-SESSION-01`): one OpenCode session serves
   every model role of ONE execution generation. Verified live: architect + both Lead
   attempts + smith + post all ran in `ses_f636a0792ffe…` (distinct sessions for that
   generation: 1), while the PREVIOUS generation's session stayed separate, so
   generations cannot share a desk. The worktree marker `.forge-session.continue` holds
   the id and each role pins it with `--session <id>`; `FORGE_SESSION_CONTINUITY=0` opts
   out; a session that fails is dropped once. Migration 174 stores the id on the run row.
5. Spend is per ROLE, not per session: the adapter reads the pinned session at launch and
   on success, and the run row carries the DELTA. Measured deltas for one generation:
   architect $0.0076, lead $0.0142 / $0.0126 / $0.0111 (cache-heavy, ~96% cache reads).
6. The Architect no longer walks the repository when a repo index is present: the latest
   live architect run made ZERO file reads (0 Read/Glob/Grep calls). The index injection
   is the fix; no runner-level guard is needed.

## Setup before any run (standing rule)

`pnpm forge:clean` FIRST. Every run reads the same control-plane tables, so another run's
leftover claim is not untidy — it can hold the single-active lock, mis-attribute a
candidate, or let a stale row answer for the live task. Then
`pnpm forge:story:reset <story> reset --force` for the story under test.

`clean` is safe on a shared control plane because it only touches claims older than
`--stale-minutes` (default 15). It ran on 2026-09-13 and found 15 engine claims left
`claimed` by earlier deaths (9 lead_pre, 4 architect, 2 fast_smith) plus 2 stale instances
and 5 open tasks — all of which the pre-clean test runs had been reading against.

The reset now also closes the story's engine claims, because a reset that leaves claims
behind is not a reset.

## The field contract, and how it fails (2026-09-13)

Every field reader keys on `(task_id, node_id, attempt)`. Five defects each made a story
that was planned CORRECTLY look unplanned, so it could never route:

1. The identity line given to the model omitted the ATTEMPT, so on a retry the model
   wrote `--attempt 1` while the runner read attempt 2: "no decision was recorded in
   fields" for a row that existed.
2. The Lead's findings read was story-wide, so earlier runs and retried attempts produced
   duplicate finding ids and the gate refused the whole context. It is now scoped to the
   live process instance and the newest attempt per node.
3. The chunk command wrote an assignment with a NULL `reasoning` and the reader treats
   that as NO assignment, so the plan read as empty. The CLI now refuses the row.
4. Assignment and chunk ids were matched case-sensitively, so `a` and `A1` were different
   assignments and the plan resolved to nothing.
5. The dispatchability scale rises with DIFFICULTY, so `worker-fit 5` means "needs a
   team". Read as praise, it HOLDs a story whose route was otherwise correct.

Root cause for all five: an incomplete or mis-keyed write is SILENT. The reader returns
null and the gate reports a missing deliverable, which reads as a model failure. When a
gate refuses, read the ROWS for that `(task, node, attempt)` before believing the message.

## Open, in priority order

1. SPLIT dogfood: never exercised end to end. SPLIT is enabled by default
   (`FORGE_SPLIT_ENABLED !== 'false'`), the parallel-claim path is wired, and the serial
   flows now complete — what is missing is a purpose-made multi-surface fixture story.
   `scripts/forge-story-reset.ts` only resets; it does not create stories.
2. Smith authorization: the serial lane treats an unreadable diff as a measurement gap
   rather than a scope miss. Deliberate today; changing it is the captain's call.
3. Completion atomicity: durable evidence is merged BEFORE the engine CAS
   (`forge-engine-runtime.ts`), so a losing worker's evidence is already committed.
4. `scripts/forge-handoff.mjs` splits list flags on commas.

