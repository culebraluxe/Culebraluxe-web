# ENG-FORGE-FACTORY-01 — Company agent infrastructure

## Goal

Stop treating Forge as an engineering cockpit that happens to run agents.
Make it the way CulebraLuxe work enters, is routed, is remembered, and
improves itself.

Cole Murray's essay (2026-09-11) and OpenInspect git are **recon only**.
Do not clone `ColeMurray/background-agents`. Do not add Cloudflare,
Modal, Daytona, E2B, Slack/Linear bots, or a second control plane.
Pirate the four buckets onto HEAD: kind-router, decision institution,
trace→work, session ROI.

This packet is the work order. The last 48h cockpit invariants stay law;
they are preconditions, not the deliverable.

## Baseline / authority

- HEAD factory already exists: `Ready` / `forge_batch` → `agent_work_item`
  → launchd worker → ledger → cockpit.
- Lanes already exist: Scout / Architect / Smith / Inspector / Assay
  (`docs/agent/packets/ENG-FORGE-LANES.md`). Do not add a sixth personality.
- DeepSeek = volume-lab. Grok = judgment-lab. Do not collapse them.
- Git packets at `docs/agent/packets/` are how Grok sees work.
- Neon Story Board is execution-control state
  (`docs/agent/STORY_EXECUTION_CONTRACT.md`).
- OpenInspect: https://github.com/ColeMurray/background-agents — hosted
  session + sandbox product. Commodity. Out of bounds.

### Reuse (do NOT rebuild)

- `forge_batch` / `forge_batch_item` + `fireStagingBatch` / `fireDueForgeBatches`
- `agent_work_item` + `agent_work_item_dispatch` — the only dispatch path
- `withdrawQueuedAgentWork`
- launchd wrapper fail-closed `git pull --ff-only origin main`
- packet lint + silent-failure hunter + path-exists map lint
- `app_error` / move-trace / `forge_engine_task_execution` / run notes
- `sessionFromStory` / `sessionFieldsFromGitPacket` / `buildLaneEnqueue`
- probes: `probe-batch-sync.ts`, `probe-batch-schedule.ts`, `probe-engine-withdraw.ts`
- `docs/agent/MEMORY.md` as **incident log only** after Phase 2 — not the store

## The idea (why this is not a fix-it ticket)

A factory that only moves website stories is a department tool.
A factory that owns kinds, decisions, and failed-session follow-up is how
the company works together.

Three new objects. One existing queue. No new harness.

```
signal (board | batch clock | hunter | later: CRM/WhatsApp)
  → kind + policy
  → forge_batch_item / Ready work item
  → lane (already exists)
  → MUST load active decisions before acting
  → traces
  → assay/hunter may write the NEXT work item
```

## Object 1 — Kind router

Every piece of work carries a `kind` and a `model_policy` before Lead speaks.

Kinds (closed set, do not invent more in v1):

| kind       | default policy | default lane start | example                         |
|------------|----------------|--------------------|---------------------------------|
| `qa`       | cheap          | Scout              | "what does X do"                |
| `fix`      | cheap          | Scout              | silent-failure, broken write    |
| `feature`  | judgment       | Architect          | new surface                     |
| `crm`      | cheap          | Scout              | HubSpot / WhatsApp follow-up    |
| `judgment` | judgment       | Architect          | constitution, scoring, HOLD     |
| `learn`    | cheap          | Assay              | hunter → packet / batch row     |

Rules:

1. Night / `fireDueForgeBatches` defaults `model_policy=cheap` unless the
   row says `judgment`.
2. Kind is stored on the batch item **and** copied onto the work item.
   The worker log line includes both.
3. Kind picks the first lane. It does not replace Lead's SMITH/SPLIT
   decision once Architect has spoken.
4. No model-picker science. Two policies only: `cheap` | `judgment`.
   Map those onto existing provider/model names in one table in code.
   Do not expose Fable vs X vs Y in the cockpit.

### Schema (additive)

- `forge_batch.model_policy` `cheap|judgment` default `cheap`
- `forge_batch_item.kind` `qa|fix|feature|crm|judgment|learn` default `fix`
- `agent_work_item.kind` + `agent_work_item.model_policy` copied at dispatch

If a column already exists under another name, reuse it. Do not dual-write.

## Object 2 — Decision institution

This is the knowledge store that outlives the agent. Not MEMORY.md.
Not a wiki. Not OpenContext. Not a neural net.

One Neon table: `forge_decision`.

```
id
key              text unique     -- stable slug, e.g. batch-table-is-job-stream
statement        text            -- one sentence, present tense
status           text            -- candidate | active | superseded
owner            text            -- human or lane that promoted it
evidence_sha     text            -- git sha or probe id that proved it
supersedes_id    uuid null
source           text            -- packet | hunter | incident | captain
domain           text            -- forge | crm | web | ops
created_at / promoted_at / superseded_at
```

Rules:

1. Scout may insert `candidate` only.
2. Architect or captain promote to `active`.
3. Smith **must not** insert or promote. Smith may read `active`.
4. Inspector may flag an active decision as stale (status stays active;
   a `learn` work item is opened). Only Architect/captain supersede.
5. Before Lead or Smith act, the runner injects the active decisions for
   the story's domain (cap 20, newest promoted first). This replaces
   "read MEMORY.md and summarize."
6. MEMORY.md remains the incident narrative. A decision that only lives
   in MEMORY is not in force.
7. Git mirror: `docs/agent/decisions/<key>.md` written on promote so Grok
   can read without Neon. Same fields. Packet lint: a decision file
   without a matching active row (or a row without a file after promote)
   is debt, not a silent split-brain. v1 may be Neon-only if the git
   mirror slips — then say so in the run notes. Prefer both.

Seed v1 with the seven factory invariants already proven on PROD (table
is the job stream; git-pull fail-closed; silent refusal is a defect;
abandoned ≠ running; intent ≠ status; withdraw is real; maps cannot cite
dead paths). Those become `active` decisions, not more cockpit comments.

## Object 3 — Traces write work

Assay already exists. Give it a job that is not "run these tests."

On each unattended worker pass, after `fireDueForgeBatches`:

1. Run the silent-failure hunter + stale-claim query against a bounded
   window (since last successful `learn` run, max 24h).
2. If new-hits exceed baseline **and** no open `learn` item already
   covers that pattern key, insert a `forge_batch_item` kind=`learn`
   into the current staging batch **or** open a `Ready` work item if
   staging is empty and the pattern is `P0` (silent write / abandoned
   claim / digest without row).
3. The `learn` story packet is generated from a template:
   pattern, first/last seen, evidence ids, proposed decision or fix
   surface. Lead/Architect still decide SMITH vs HOLD. Assay does not
   ship code.

Cap: at most **one** auto-filed `learn` item per worker pass.
Never auto-merge. Never auto-promote a decision.

## Object 4 — Session ROI (thin)

Do not build a finance system.

On work-item finish, persist:

- `kind`, `model_policy`
- token/cost if the harness already records it (reuse `ENG-FORGE-COST-01`
  / run-usage surfaces; do not invent a second meter)
- `result_status`
- wall time

Cockpit: last 7 days, count and cost by kind. That is enough to stop
token-maxxing the night batch. Attribution taxonomy beyond kind is a
later packet.

## Preconditions (already true — do not re-litigate)

- Table is the job stream.
- Unattended path fails closed on git.
- Silent refusal is a defect class.
- Abandoned claim ≠ running (same window as cleaner).
- Intent ≠ status.
- Withdraw cancels `Ready`, reports `Claimed`/`Running`.
- Maps cannot cite dead paths.

If any of these have regressed, HOLD this packet and repair. Do not
stack new objects on a lying board.

## Out of bounds

- `ColeMurray/background-agents` as a dependency or deploy target
- OpenContext / Understand Anything / turbovec installs
- New daemon next to launchd
- New web UI besides columns/fields on the existing cockpit
- MEMORY.md rewrite or wiki import
- Neural memory, Docker memory boxes
- Auto-merge, auto-push, production app migrations
- More than two model policies
- More than six kinds
- HubSpot/WhatsApp as a live signal in v1 (kind `crm` exists so the
  door is named; the adapter is a later packet)

## Phases (commit separately)

### Phase 1 — Kind + policy on the existing table

Schema + copy onto work item at dispatch + worker log + cockpit shows
kind on the batch roster + night default `cheap`.

Acceptance:
- Stage two items, one `fix/cheap`, one `judgment/judgment`.
- Fire table. Work items carry kind+policy. Worker log names them.
- `fireDueForgeBatches` without an override writes `cheap`.
- Probe, net zero, on PROD or DEV with restore.

### Phase 2 — Decision table + inject

`forge_decision` + seed seven invariants + runner injects active
decisions into Lead/Smith context + Smith cannot promote.

Acceptance:
- After a cold session, Smith prompt contains the seeded decisions
  (targeted test on the injector, not a vibe check).
- Scout insert stays `candidate`. Promote path is Architect/captain only.
- MEMORY.md is not read as authority in the injector.

### Phase 3 — Learn loop

Hunter → at most one `learn` item per pass → template packet.

Acceptance:
- Fixture: baseline + one new silent-failure pattern → exactly one
  `learn` row/work item, second pass does not duplicate.
- Assay does not commit.

### Phase 4 — ROI strip

Finish writes kind/policy/cost/result. Seven-day by-kind rollup on
cockpit ENGINE panel.

Acceptance: one finished cheap fix and one judgment feature show as two
rows in the rollup. Skip this phase if cost is not already on the run.

## Stop conditions

1. Kind routing starts choosing providers per token. HOLD. Two policies.
2. Decision table becomes a blog. HOLD. One sentence per row.
3. Learn loop files more than one item per pass or ships code. HOLD.
4. Anyone adds OpenInspect packages. HOLD and revert.
5. Board and table disagree after Phase 1. Repair before Phase 2.

## Test strategy

- Targeted suites per phase (dispatch copy, injector, hunter de-dupe).
- One probe per phase, net zero.
- Full workflow_app suite only after Phase 2 injector is green.
- Do not run full suite after Phase 1 schema.

## Skills

workflow, planner

## Loop

intent: grow
loop: 1/3

## Test mode

SCOPED

## Assay commands

- (Phase 1) targeted test for kind/policy copy at dispatch
- (Phase 2) targeted test that injector reads `forge_decision` status=active and not MEMORY.md
- (Phase 3) targeted test that hunter new-hit files one learn item and does not duplicate

## Final report

Commits · files · schema applied where · whether seed decisions are active
· whether a live worker log shows kind+policy · whether learn de-dupe held
· stop-signs · explicit confirmation OpenInspect was not imported.
