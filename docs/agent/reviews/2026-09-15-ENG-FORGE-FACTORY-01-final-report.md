# ENG-FORGE-FACTORY-01 — Final report

Packet: `docs/agent/packets/ENG-FORGE-FACTORY-01.md` (company agent infrastructure: router, decisions,
traces → work). Phases 1–4 implemented, migrated in DEV **and** PROD, and live in production on `ce553e9`.

Written 2026-09-15. This file answers the packet's Final report section item by item, with the check that
produced each answer.

## Commits

| Commit | Phase / part |
| --- | --- |
| `5dfbfce` | the packet itself |
| `249a3e6` | the gates that make the packet checkable (lint + hunter) |
| `f462e76` | PIRATE-01: ranked scope manifest, regenerable vendor blocks, checkable evidence |
| `06f6d3c` | board repair (the five V5 stories) — the precondition of stop-condition 5 |
| `95b4d38` | **Phase 1** — kind + model policy, copied where both facts are in hand |
| `6ebd6b4` | **Phase 2** — decisions are rows; the write policy is the whole point |
| `cbbdfc0` | **Phase 3** — traces write work; three rules get three kinds of enforcement |
| `db3696d` | **Phase 4** — the thin ROI strip (no new meter) + the edge build fix |
| `ce553e9` | the release fix: `turbopack: {}` (Next 16 refuses a webpack config with no turbopack config) |
| `50f4ec9` | register updated |

## Files

Phase 1 `db/migrations/179_forge_kind_policy.sql`, `lib/forge-kind.ts`, `scripts/set-story-status.ts`,
`workflow_app/tests/forge-kind-routing.test.ts`. Phase 2 `db/migrations/180_forge_decision.sql`,
`lib/forge-decision.ts`, `db/forge-decision.ts`, `scripts/forge-decision.ts`,
`workflow_app/tests/forge-decision.test.ts`. Phase 3 `db/migrations/181_forge_learn_pattern.sql`,
`lib/forge-learn.ts`, `db/forge-learn.ts`, `agent-runtime/learn-loop.ts`, `scripts/forge-learn.ts`,
`scripts/probe-learn-dedupe.ts`, `workflow_app/tests/forge-learn.test.ts`, plus `lib/artifact-file.ts`.
Phase 4 `lib/forge-roi.ts`, `db/forge-roi.ts`, `scripts/forge-roi.ts`,
`components/portal/tech/engineering-line/EngineeringQueuesPage.tsx`, `app/portal/tech/page.tsx`,
`workflow_app/tests/forge-roi.test.ts`, and `package.json` (`forge:roi`).

## Schema applied where

All three migrations are recorded in **both** environments — this is the ledger, not an assertion:

```
Tue Sep 15  prod  db/migrations/181_forge_learn_pattern.sql  — ENG-FORGE-FACTORY-01 Phase 3: learn_pattern_key + the open-pattern unique index
Tue Sep 15  dev   db/migrations/181_forge_learn_pattern.sql  — ...
Tue Sep 15  prod  db/migrations/180_forge_decision.sql       — ENG-FORGE-FACTORY-01 Phase 2: forge_decision table + the seven seeded factory invariants
Tue Sep 15  dev   db/migrations/180_forge_decision.sql       — ...
Tue Sep 15  prod  db/migrations/179_forge_kind_policy.sql    — ENG-FORGE-FACTORY-01 Phase 1: kind + model_policy columns (batch, batch item, work item)
Tue Sep 15  dev   db/migrations/179_forge_kind_policy.sql    — ...
```

`pnpm db:migrations` → prod recorded 49, dev recorded 15. `pnpm db:parity` green.

## Seed decisions active

Yes — `pnpm forge:decision list` against PROD returns 7, all `active`, all `forge · captain · 2026-09-15`:
`batch-table-is-job-stream`, `unattended-path-fails-closed-on-git`, `silent-refusal-is-a-defect`,
`abandoned-claim-is-not-running`, `intent-is-not-status`, `withdraw-is-real`,
`maps-cannot-cite-dead-paths`. The injector reads status=active rows, not `MEMORY.md` (unit-tested in
`workflow_app/tests/forge-decision.test.ts`).

## Does a live worker log show kind + policy

**Not yet observed — the honest answer.** The copy and the log line are unit-tested, and the columns are in
place in both environments, but no engine claim has happened since Phase 1 landed: the board currently
reports `batched 0 · handed to the engine (Ready) 0 · work bench 0`. The first post-Phase 1 claim is what
will print it. Recorded in `docs/agent/TECH-DEBT.md` rather than claimed here.

## Did learn de-dupe hold

Yes, on all three counts, measured in one dry run:

- **cap** — `pnpm forge:learn --dry-run`: 40 changed code files scanned, 5 candidates, `would file` exactly
  one, four `deferred (cap is one per pass)`.
- **de-dupe** — `scripts/probe-learn-dedupe.ts` PASS: a second attempt at the same `learn_pattern_key` is a
  no-op, backed by the partial unique index on open patterns (migration 181).
- **does not ship code** — `agent-runtime/learn-loop.ts` and `scripts/forge-learn.ts` have no commit, push or
  merge path; the loop's output is a work item.

## Stop-signs

None hit. Two were checked mechanically in this report's run:

1. **Kind routing choosing providers per token** — no. One table, policy is copied at dispatch; no provider
   branching in application code.
2. **The decision table becoming a blog** — no. One-sentence discipline enforced at creation *and* at
   injection; 7 rows total.
3. **Learn loop filing more than one item per pass, or shipping code** — no (see above).
4. **Anyone adding OpenInspect / Cloudflare / Modal / Daytona / E2B** — no. `package.json` has no such
   dependency, and a tracked-file grep for `openinspect|background-agents|ColeMurray|daytona|e2b` returns no
   reference outside `docs/`. (One false positive remains in the tree: a hex session id in
   `workflow_app/tests/persistence/hot-patch-runtime.test.ts` contains the substring `e2b`.)
5. **Board and table disagreeing after Phase 1** — repaired *before* Phase 2 (`06f6d3c`), and today
   `pnpm forge:batch:status` reports `board vs table: agree`.

## OpenInspect was not imported

**Confirmed.** The repository was recon only. Nothing from it is vendored, imported, or depended on — no
package, no source file, no runtime call. What the packet took from that line of research is conceptual
(ranked scope manifests, evidence you can verify, regenerable generated blocks), and it was rebuilt against
this repo's own tables and seams.

## Release evidence

Production serves `ce553e9`: `/api/build-info` → `V2 · ce553e9 · built 2026-09-15T09:31:25Z`;
`pnpm smoke:prod --expect-sha ce553e9` → 4/4 (stamp, home, buyers inventory, sha equality); `/portal/tech`
→ 307 to `/login`, which is what shows the edge middleware and the edge-safe instrumentation load.

## What this report does not prove

Anything that needs a live attempt. The unproven list is short and is tracked in `docs/agent/TECH-DEBT.md`:
kind+policy never yet printed by a real claim, the learn loop not yet exercised by an unattended PROD run,
one `unrecorded` ROI row until the first post-Phase 1 attempt lands, `cost_usd` mostly empty (no invented
rate), per-card kind chips deferred, and two build paths (Turbopack for the release, webpack for QA) that
still need to be reconciled into one.

**Phase 4's own acceptance criterion in particular is not demonstrated.** The packet asks that "one finished
cheap fix and one judgment feature show as two rows in the rollup". That needs two finished runs, and there
has not been one since Phase 1 landed — so what is verified is the strip rendering, the by-kind query, and
the copy at dispatch, not the two-row rollup itself. The packet also allows skipping Phase 4 when cost is
not already on the run; `cost_usd` is still mostly empty because no rate was invented, which is why this
phase was built as a thin strip over existing data rather than a new meter.

