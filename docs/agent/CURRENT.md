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

**The machine is Rust.** The port's TypeScript spec — the 465-file restored test estate and the retired
production files they assert against — left the tree on 2026-10-02 and is archived outside it at
`~/Documents/forge-legacy-ts-archive-2026-10-02/` (byte-identical copy plus `legacy-2026-10-02.zip`, taken at
`5b4e5aa4`; counts, reason and how to get it back are in `docs/agent/DEAD-TS-DOWNSIZE.md` §5.2). Nothing live
reads it: the last measurement before it went, `cargo check --workspace` with `legacy/` physically absent,
exited 0. Read the archive through `docs/agent/MAP-engine.md` and `docs/agent/LEGACY-TEST-PARITY.md`, which
are the accounting.

1. Engine: `rust/forge`, driven by `pnpm forge:engine` (`cargo run -p forge --bin forge`), with
   `forge-worker`, `forge-task` and `re-workflow` beside it for the worker, single-task and RE paths
   (`rust/forge/src/bin/`). The definition is `rust/forge/definitions/FORGE_SDLC-v6.xml`, embedded in the
   binary (`rust/forge/src/engine/xml.rs:482`, `include_str!`) and parsed by the production parser
   (`wf_definition__013`). The `legacy/` copy is in the archive and is read by nothing — measured
   2026-10-02, no file under `rust/` names that path.
2. Roles are **seven** services in `rust/forge/src/roles/`, each implementing `roles/service.rs`'s
   `AbstractForgeService` (descriptor, runner, hooks) and each answering `roles/hooks.rs`'s `ForgeRoleHooks` —
   one lane's reading of its own turn: `ScoutService` (`forge.scout`), `ArchitectService` (`forge.architect`),
   `LeadService` (`forge.lead`), `SmithService` (`forge.smith`), `InspectorService` (`forge.inspector`),
   `AssayService` (`forge.assay`, in `roles/qa.rs`), `DevOpsService` (`forge.devops`, in `roles/dev_ops.rs`).
   Which service owns a node is read from the definition, not from Rust: `FORGE_SDLC-v6.xml` binds
   `service="forge.smith"` on each task-node (`rust/forge/src/engine/service_binding.rs`), a human gate carries
   none, and `ForgeLaneServices` composes the seven once into `roles/registry.rs`'s `ForgeServiceRegistry`. Work
   reaches them as durable jobs: `rust/forge/src/engine/job.rs` turns a READY task into a `forge.role` job (lease,
   heartbeat, attempts) and `rust/forge/src/bin/forge.rs` drives it with `WorkflowJobService` plus that registry.
   The shared `rust/forge/src/roles/lifecycle.rs` asks the lane rather than switching on a node id and stays the
   decider, with the gate's kinds, deliverable table and effect ports in `rust/forge/src/engine/phase.rs`. (The
   pair `legacy/workflow_app/forge/agents/role-agents.ts` / `legacy/workflow_app/forge/forge-phase-agent.ts` is
   retired with the rest of `legacy/workflow_app/forge/`.)
3. A lane runs through the OpenCode harness, which is Rust too (`rust/forge/src/engine/opencode.rs`,
   `rust/forge/src/engine/opencode_agents.rs`, `rust/forge/src/engine/opencode_client.rs`,
   `rust/forge/src/engine/opencode_events.rs`), in the ONE workspace the engine provisions for a run
   (`rust/forge/src/engine/worktree.rs`: branch `agent/…`, `culebraluxe-forge-worktrees`). There is no
   per-lane tree, and a tracked file that introduces one fails a test — AGENTS.md, "NO TREES. EVER.".
   The TypeScript `agent-runtime/` harness is gone — with its 40-file test suite, deleted 2026-10-02
   (`legacy/agent-runtime`, `legacy/services`, `legacy/lib`, 45 files; the rail is `arch_boundary__012`) —
   and the rest of `legacy/` followed it out of the tree the same day (432 files, 79,479 lines, archived at
   `~/Documents/forge-legacy-ts-archive-2026-10-02/`). What is left in the repository is five JavaScript
   files and no TypeScript at all.

## The one doctrine that matters

The failure mode is the CHANNEL, not the parser. A decision must never live only in a
model's chat reply.

1. Decisions travel in rows: `forge_role_contract` (migration 170) for the decision,
   `forge_role_plan_chunk` / `forge_role_assignment` (171) for the plan.
2. Reply markers (`LEAD_ROUTING:`, `FORGE_ARCHITECT_HANDOFF:`) remain as the FALLBACK for
   rows that were never written. Fields win; text is the fallback.
3. The Lead decision has ONE seat in the live engine: it is taken once, in the PRE phase, and a reply
   that restates one during any other Lead turn is stripped rather than believed
   (`rust/forge/src/roles/lead.rs`), with the routing assay (`LeadProposal`, `RoutingReview`) and the
   bench cap (`bench_intent_errors`) in `rust/forge/src/engine/role_slice.rs`. A refusal cannot be
   re-reviewed by a second evaluator because there is no second resolver — the retired
   `legacy/workflow_app/forge/lead-proposal-resolve.ts` was that seat in the TypeScript engine, and the
   assertions behind it outlive it as tests in `rust/forge/src/engine/role_slice.rs`.

## Measured facts (do not re-derive these)

1. One role's model call is about 39 seconds; role wall clock measured 52 seconds on
   2026-09-13. Harness startup is 0.30s and config load 0.55s, so startup is not the cost
   — inference is.
2. The `rtk` shim used to fork-bomb every tool call (4,627 processes, load 34, about 80s
   blocked per call, every parent asleep on its child). The fix lived in `forge-tool-seams.ts`,
   retired with the TypeScript harness — `rtk` survives in the tree only as a skill NAME
   (`rust/forge/src/engine/opencode_agents.rs`), and the live launch path
   (`rust/forge/src/engine/opencode_client.rs`) spawns the harness with no PATH rewriting of its own.
   History of the old harness: if a shim returns to the launch path, this is the failure to expect.
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

1. SPLIT dogfood: never exercised end to end. The lane is wired (`rust/forge/src/engine/split_join.rs`,
   `rust/forge/src/engine/commands.rs`'s `RUN_SMITH_SPLIT`, and the SPLIT count check in
   `rust/forge/src/roles/lead.rs`) and the serial flows now complete — what is missing is a purpose-made
   multi-surface fixture story. Admission is the Lead's decision plus the bench cap
   (`rust/forge/src/engine/role_slice.rs`, `bench_intent_errors`): `FORGE_SPLIT_ENABLED` has **no reader
   in `rust/`** (it survives in older notes only), so do not believe a note that says this lane is
   switched on or off by it. `pnpm forge:story:reset` (`rust/cli/src/forge/reset.rs`) only resets; it
   does not create stories.
2. Smith authorization: the serial lane treats an unreadable diff as a measurement gap
   rather than a scope miss. Deliberate today; changing it is the captain's call.
3. Completion atomicity: the live unit is `rust/forge/src/engine/completion.rs`'s
   `apply_completion_unit` — claim, then merge (`rust/forge/src/engine/db_ledger.rs`'s `merge_evidence`
   → `ForgeEngineDao::merge_workflow_evidence`), then finalize — so the receipt is taken BEFORE the
   evidence is merged, not after. This item used to cite `forge-engine-runtime.ts`, retired with the
   TypeScript engine, and the order it described (evidence merged before the engine's compare) is **not**
   the live order: re-measure before acting on it.
4. `scripts/forge-handoff.mjs` split list flags on commas; that script is retired with the TypeScript
   tooling. The live comma-splitting surfaces are the marker parsers
   (`rust/forge/src/engine/role_mapping.rs`, `rust/forge/src/engine/smith_candidate.rs`,
   `rust/forge/src/engine/architect.rs`), so a multi-value flag has to be checked there.

