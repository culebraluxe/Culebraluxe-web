# MAP — the engine and the harness (where to open a file)

Two layers, often confused. **`workflow_app/` is the engine**: it owns the workflow definition, the
phases, the gates, the evidence, the publish. **`agent-runtime/` is the harness**: how ONE role is run —
its prompt, its lane, its skills, its write policy, its candidate handoff. The engine decides; the harness
executes one assignment.

Status and history are never here — they are rows in Neon (see `ORIENTATION.md`).

## Engine (`workflow_app/`)

| Open this | To answer |
|---|---|
| `workflow_app/definitions/FORGE_SDLC-v6.xml` | What phases and nodes does a run actually have? |
| `workflow_app/forge/forge-executor.ts` | Who advances the workflow, and on what? |
| `workflow_app/forge/agents/role-agents.ts` | What each role is asked to produce, per phase |
| `workflow_app/forge/agents/architect/assess.ts` | How an Architect handoff is judged (it can refuse) |
| `workflow_app/forge/agent-runtime-role-runner.ts` | How one role's attempt is launched, in which worktree, with what contract |
| `workflow_app/forge/agents/forge-phase-agent.ts` | The parent gate: what a phase result must prove before it is accepted |
| `workflow_app/forge/evidence-gate.ts` | Which evidence a phase requires |
| `workflow_app/forge/forge-facts.ts` | The gate facts projected for a run (a stale snapshot here has cost real hours) |
| `workflow_app/forge/failure-classifier.ts` | How a failure is classified (defect vs. environment vs. capacity) |
| `workflow_app/forge/forge-release-receipt.ts` | The receipt rule: **derived from a real deployment signal, never asserted** |
| `workflow_app/forge/db-release-executor.ts` | Publish, integration with a moving `main`, and the verification after it |
| `workflow_app/forge/forge-claim-blocker.ts` | Why a claim was refused (the single-active lock) |
| `workflow_app/forge/forge-dispatch-gate.ts`, `forge-dispatch-seam.ts` | What may be dispatched, and by what trigger |
| `workflow_app/flight-recorder-read.ts` | The recorder's read model (what an instance actually did) |
| `workflow_app/forge/forge-convergence*.ts`, `workflow_app/forge/forge-alerts/` | Convergence projection and alerting |

## Harness (`agent-runtime/`)

| Open this | To answer |
|---|---|
| `agent-runtime/skills.ts` | Which skill packs exist (`KNOWN_SKILLS`) and how unknown ones are dropped |
| `agent-runtime/lane-policy.ts` | What each lane (smith / inspector / assay / night) is told, and its limits |
| `agent-runtime/write-policy.ts` | Who may commit; non-builder commits are revoked |
| `agent-runtime/repo-context.ts` | How repo context is assembled for a role |
| `agent-runtime/assay-plan.ts`, `assay-evidence.ts`, `assay-arithmetic.ts` | How Assay plans and grades evidence |
| `agent-runtime/git-packet.ts`, `candidate-assay-handoff.ts`, `accepted-candidate-publish.ts` | The packet → candidate → publish chain |
| `agent-runtime/deepseek/` | The DeepSeek adapter path |
| `agent-runtime/silent-failure-patterns.ts` | The gate that fails a change introducing a swallowed failure |

## The supporting reads (Neon)

| Module | What it reads |
|---|---|
| `db/agent-work.ts` | `agent_work_item` — the engine's queue and the single-active lock |
| `db/forge-engine-task-execution.ts` | `forge_engine_task_execution` — the ledger (what ran, how it ended) |
| `db/forge-batch.ts` | `forge_batch` / `forge_batch_item` — staged and scheduled runs |
| `db/forge-hold.ts` | Durable HOLD records |
| `db/storyboard.ts` | Stories, statuses, bench, execution summaries |
| `db/app-error.ts` | Durable error capture (`app_error`) |

**Where the traps are written down:** `docs/agent/MEMORY.md` (dated, durable) and
`docs/agent/WORKFLOW-ARCHITECTURE.md` (laws, failure taxonomy, roadmap). Read the recent MEMORY entries
before changing the engine — most of them are things that already cost a night.
