# MAP - the engine and the harness (where to open a file)

Two layers, often confused.

**`rust/forge` is the engine**: it owns the workflow definition, the phases, the gates, the evidence, the roles and the
release receipts. It decides what the workflow does next, and it writes that decision to Neon.

**The harness is how ONE role is run**: its prompt, its lane, its skills, its write policy, its candidate handoff. It
executes one assignment and reports back. Today the harness is Rust too, inside `rust/forge/src/engine/` (session,
packet, worker, runner) and `rust/forge/src/roles/`. The TypeScript harness under `agent-runtime/` is retired - read it
for intent, never as a live path.

Status and history are never in a file: they are rows in Neon (`agent_work_item`, `forge_engine_task_execution`,
`forge_tool_artifact`, `storyboard_story_run`, `forge_workflow_evidence`, `app_error`, `workflow_execution_trace_event`).
To learn what a run did, ask the rows.

The workflow mechanics themselves live one crate over: `rust/core/workflow` (`WorkflowEngine`, `TxStore`, task status,
process instances). `rust/forge` is the Forge SDLC on top of it.

## The engine (`rust/forge`)

| Open this | To answer |
| --- | --- |
| `rust/forge/definitions/FORGE_SDLC-v6.xml` | What phases, nodes and gates does a run actually have? |
| `rust/forge/src/engine/definition.rs` | How the definition is parsed; `forge_sdlc_graph()` is a test fixture, not production. |
| `rust/forge/src/engine/executor.rs` | Who advances the workflow, and on what. The production drive refuses the synthetic runner. |
| `rust/forge/src/engine/runtime.rs` | How an instance is started, completed and stepped through the workflow crate. |
| `rust/forge/src/engine/phase.rs`, `topology.rs`, `role_mapping.rs` | How a phase maps to roles and to the commands it may run. |
| `rust/forge/src/engine/facts.rs` | The gate facts projected for a run - **booleans default FALSE, absent enums are omitted**: it fails closed. A stale read here has cost real hours. |
| `rust/forge/src/engine/evidence_gate.rs` | Which evidence a phase must have before its result is accepted. |
| `rust/forge/src/engine/first_violation.rs`, `failure.rs` | How a failure is classified (defect vs environment vs capacity). |
| `rust/forge/src/engine/claim_blocker.rs`, `agent_work.rs`, `stale_claim.rs` | The single-active lock, the claim queue, and the recovery path for a claim older than 15 minutes. |
| `rust/forge/src/engine/qa_assert.rs`, `qa_adjudicate.rs`, `qa_classify.rs`, `qa_repair.rs` | What QA must assert, who adjudicates the verdict, and how a repair is requested. |
| `rust/forge/src/engine/release_receipt.rs`, `release.rs`, `git_publish.rs`, `deploy.rs` | The receipt rule: **derived from a real deployment signal, never asserted** - and the publish that must survive a moving `main`. |
| `rust/forge/src/engine/dispatch.rs`, `ready_gate.rs`, `serial_doors.rs` | What may be dispatched, by which trigger, and what may run at the same time. |
| `rust/forge/src/engine/worktree.rs`, `workspace_id.rs` | The one worktree the engine provisions for a run - not a per-lane tree. |
| `rust/forge/src/engine/spend_cap.rs`, `turn_budget.rs`, `self_heal.rs` | Cost and turn limits, and how a run heals itself. |
| `rust/forge/src/engine/hold.rs`, `hold_resolve.rs`, `decisions.rs` | Durable HOLD records and how one is resolved. |
| `rust/forge/src/engine/observer.rs`, `learn.rs` | The Flight Recorder trace and what a finished run teaches. |
| `rust/forge/src/engine/packet.rs` | The canonical task text handed to a role. |
| `rust/forge/src/engine/neon_sql.rs`, `db_writer.rs` | How the engine talks to Neon - the engine's own narrow door, not the service kernel. |
| `rust/forge/src/roles/` | What each role (`scout`, `architect`, `lead`, `smith`, `qa`, `dev_ops`) is asked to produce. |
| `rust/forge/src/bin/forge.rs`, `forge_worker.rs`, `forge_task.rs`, `re_workflow.rs` | The binaries: drive, worker, single task, RE workflow. |

## The harness gates (`rust/cli/src/forge` and `pnpm`)

| Command | Gate |
| --- | --- |
| `pnpm forge:harness` | All of the harness checks below, in one call - the one to run before pushing |
| `pnpm forge:packet-lint` | Harness lint: evidence paths that must exist, vendor blocks that must match a fresh render, the rules in `AGENTS.md` that back them |
| `pnpm forge:sync-agents` | Writes the vendor pointer blocks; fails when one has drifted |
| `pnpm forge:manifest [--check]` | Builds (and validates) the scope manifest for a story |
| `pnpm forge:silent-failure-gate` | Fails a change that swallows a failure |
| `pnpm broken:ts:sweep` | Dead-TypeScript counts; fails when the tree and the inventory disagree (`docs/agent/BROKEN-TS-INVENTORY.md`) |

`rust/cli/src/forge/` holds `lint.rs`, `citations.rs`, `vendor_block.rs`, `sync_agents.rs`, `decision.rs` and
`secret_shapes.rs`.

## The rows (Neon) the engine reads

| Table | What it holds |
| --- | --- |
| `agent_work_item` | The queue and the single-active lock |
| `forge_engine_task_execution` | The ledger: what ran, how it ended |
| `forge_tool_artifact` | Tool output and evidence attached to a run |
| `storyboard_story_run` | Runs per story, their status and summaries |
| `forge_workflow_evidence` | The evidence a gate accepted |
| `workflow_execution_trace_event` | The Flight Recorder trace (observer-only, joined by process-instance id) |
| `app_error` | Durable error capture, for every failure that reaches a seam |

`pnpm forge:doctor` answers "is anything in flight" from those rows. `pnpm forge:clean` cancels stale work and aborts
stale instances - and note it resolves to the **production** database and runs `--force`: it is a production action, not
local hygiene.

## Driving it, safely

- **Forge runs against PROD only.** A run may not flip the environment; that is the engine's design, not a default to
  argue with (`docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` §0).
- **Before any engine run or test**, clear stale state: `pnpm forge:clean` (production action, needs the Captain's go)
  then `pnpm forge:story:reset <story> reset --force`. A read taken against another run's leftovers is not evidence.
- **Before a manual drive, stop the scheduler** (`pnpm agent:scheduler:stop`), confirm nothing is in flight
  (`pnpm forge:doctor`: `open engine tasks: 0`, `active claims: 0`), do the work, then resume
  (`pnpm agent:scheduler:install`). It ticks every 180s, so a check and an edit are not atomic.
- **Do not read logs for status.** Ask the rows. One query, one answer, then stop.

## Where the traps are written down

`docs/agent/MEMORY.md` (dated, durable) and `docs/agent/WORKFLOW-ARCHITECTURE.md` (laws, failure taxonomy, roadmap).
Read the recent MEMORY entries before changing the engine: most of them are things that already cost a night.

One more rule that is older than Forge and still settles arguments: **the domain is Rust.** If a change decides what is
true about a client, deal, contract, property or workflow - or reads or writes the database - it is Rust, and the
TypeScript under `legacy/` and `agent-runtime/` is reference, never a live path.