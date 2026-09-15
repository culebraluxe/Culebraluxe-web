# ORIENTATION — the map (start here)

**What this is:** a pirate map for whoever arrives next — a new agent, a fresh context, or the captain
after a week away. It answers "what is this app, where does everything live, what do I run" in paths and
commands you can check. It is **not** a spec and **not** a status page: story status, engine state and
batch state live in Neon (see *Where to look for X*), never here.

Three sentences to carry:

1. **State is in rows.** A decision that lives only in a chat reply does not exist.
2. **A gate that cannot fail is decoration.** Rules get enforced by gates (`pnpm forge:packet-lint`,
   Assay, write-policy), not by prose.
3. **A refusal must name its cause.** Never let a failure become a silent 500, a bare `catch`, or an
   unexplained HOLD.

## The app in one paragraph

CulebraLuxe: a luxury property site plus a private portal, built as a Next.js App Router application on
Vercel, with Neon/Postgres for business and property data, Mux for video, and Google Maps for locations.
The portal holds the operational surfaces (CRM, contracts, forms, accounting, calendar, clients, deals,
media). On top of it runs **Forge** — a workflow engine that executes AI agent roles against stories to
produce code, and whose state is itself in Neon (work items, tasks, runs, evidence).

## The layers

| Layer | Path | What lives there |
|---|---|---|
| Routes | `app/` | App Router screens; the portal is `app/portal/*` (~40 screens); APIs are `app/api/*` |
| UI | `components/` | React components; portal UI under `components/portal/` |
| Domain logic | `services/<domain>/` | Business rules per domain — `<domain>-service.ts`, `repository.ts`, `index.ts`. See `MAP-services.md` |
| Data access | `db/` | Repositories, `db/database-gateway.ts`, error capture (`db/app-error.ts`) |
| Cross-cutting | `lib/` | Auth (`lib/auth/`), error-capture seams, storyboard projections (`lib/storyboard-data.ts`), move rules (`lib/story-moves.ts`), sorter cards (`lib/sorter-board.ts`) |
| The engine | `workflow_app/` | Forge: phase agents, gates, executors, the topology XML. See `MAP-engine.md` |
| The harness | `agent-runtime/` | How one role runs: skills, lane prompts, write policy, assay planning, candidate publish |
| Operations | `scripts/` | Worker entry, migrations, probes, lint, the local build/deploy scripts |
| This map | `docs/agent/` | `ORIENTATION.md` (this), `MEMORY.md` (durable decisions), `docs/agent/packets/` (per-story), `docs/agent/skills/` (packs) |
| Tests | `workflow_app/tests/`, `testv2/`, `agent-runtime/*.test.ts` | Engine + app suites |

## The three stores — and who is authoritative for what

- **Neon (facts).** Stories, statuses, the engine ledger, batches, holds, errors. If the question is
  "what is true right now", Neon answers it. Nothing else may claim to.
- **Git (code and this map).** The code, and the docs that describe it, travel in the same commit — which
  is why this map is a file and not a wiki page: it can be diffed, reviewed and gated.
- **Everything else is a view.** The Cockpit, the board, `forge:batch:status`, `/api/build-info`, any
  generated page — all **derived** from the two stores above. A view may be approximate; it must never be
  a second place where truth is edited.


## The commands that matter

```
pnpm forge:batch:status      # read-only truth: batch table, engine queue, bench, board-vs-table agreement
pnpm forge:packet-lint       # harness gate: packets, skills, MEMORY, allowlists (fails on new violations)
pnpm forge:clean             # clear stale engine claims BEFORE any engine run or test
node --import tsx --test workflow_app/tests/*.test.ts    # app/engine suite
pnpm test:agent-runtime      # harness suite (30 files)
pnpm db:parity               # DEV vs PROD schema drift (a release gate)
pnpm db:migrate <file> <dev|prod> --note "…"             # schema change, with a recorded ledger row
git diff --check && pnpm exec next build --webpack       # per AGENTS.md
```

Production is built **locally** and deployed prebuilt (Node 24 is required):

```
bash scripts/vercel-build-prod.sh    # builds .vercel/output, stamps the commit; deploys nothing
bash scripts/vercel-deploy-prod.sh   # needs main + a clean tree + a matching stamp; VERIFIES the live sha
curl -s https://www.culebraluxe.com/api/build-info      # {"version","sha","builtAt"} — what is actually live
```

⚠️ **Never run a plain `next build` between `vercel-build-prod.sh` and `vercel-deploy-prod.sh`.** It
overwrites `.next` and deletes `.next/required-server-files.json`, which the prebuilt deploy needs; the
deploy script preflights this, but the rule is simpler than the error message.

## Where to look for X

| Question | Look here |
|---|---|
| What is the status of a story? | Neon `storyboard_story.status`; the Cockpit at `/portal/tech`; `pnpm forge:batch:status` |
| What is the engine doing / has it done? | `forge_engine_task_execution` (ledger), the Flight Recorder at `/portal/tech/flight-recorder/<instance>`, `agent_work_item` (queue) |
| What is staged for a run, or scheduled? | `forge_batch`, `forge_batch_item`; the ENGINE BATCH column |
| Where did an error go? | `app_error` table, `/portal/tech/app-errors`; `instrumentation.ts` (`onRequestError`) captures every server failure with its digest |
| What is deployed right now? | `/api/build-info`; the Cockpit's version corner reads `V2 · r<commit-count> · <sha>` |
| What is this story meant to do? | `docs/agent/packets/<STORY-ID>.md` |
| Why is it like this? | `docs/agent/MEMORY.md` (durable decisions, dated), `docs/agent/WORKFLOW-ARCHITECTURE.md` |
| Where does a feature's logic live? | `MAP-services.md`, then `services/<domain>/<domain>-service.ts` |
| How does the engine work? | `MAP-engine.md`, then `docs/agent/WORKFLOW-ARCHITECTURE.md` |

## Boundaries (and why)

- **`workflow_engine/**` is not yours to edit** — captain-owned; the engine's DB ratchet lives there.
- **Never** reset PROD, copy DEV over PROD, truncate canonical history, or commit secrets/`.env.local`.
- **Forge runs execute against PROD only.** The environment is not something a run may flip.
- **Scout, Assay and Inspector never commit.** Only the Builder role commits, on the worker branch.
- **Do not special-case a listing** (e.g. Casa Luar) in application code.
- **Do not build per story what belongs to a batch.** Publishing is per story; a *release* is one build for
  many stories — the Thursday model, automated.

## Reading order for a new agent

1. This page. 2. The packet for your story (`docs/agent/packets/<STORY-ID>.md`) — it names your scope.
3. `MAP-services.md` or `MAP-engine.md` for the area you touch. 4. `MEMORY.md`'s recent entries for the
traps the last person hit. 5. Then the code.
