# Astra code review — triage map (2026-09-17)

Reviewer: **ChatGPT "Astra"**, grade **79/100**, reviewed main at `2920742a`. Delivered as 4 confirmed
bugs, 6 bug candidates, 10 improvements, 10 feature proposals.

This file is the ONE place that maps every item to what happened to it, so the same finding is not filed
twice — the same rule the engine now enforces for a fact with two writers.

Batches: **98** = defects, **99** = improvements, **100** = features. Every filed story carries its
evidence in its own notes; nothing is hidden in a private list.

## Confirmed bugs → batch 98 (each verified in source before filing)

| # | Astra finding | Story | Verified at |
|---|---|---|---|
| 1 | DEV verification checks DEV/PROD parity before PROD migration, so a new table blocks its own promotion | `ENG-FORGE-RELEASE-ORDER-01` | `release-operations.ts:198` calls `checkSchemaParity(devUrl, prodUrl)`; drift list built at `:201-208` |
| 2 | Migration SQL executes before its receipt is recorded, so a retry can repeat it | `ENG-FORGE-MIGRATION-REPLAY-01` | `release-operations.ts:92` runs `pool.query(migration.sql)` first; the `forge_migration_execution` write follows separately |
| 3 | Engine advancement, evidence and repair counters are separate writes | `ENG-FORGE-CRASH-WINDOW-01` | `forge-engine-runtime.ts:221` advances before the evidence write; a crash can undercount repairs at `forge-executor.ts:356` |
| 4 | `Z` appended to a timestamp that already carries an offset → NaN → a live claim reads as stale | `ENG-FORGE-CLAIM-CLOCK-01` | `db/forge-engine-task-execution.ts:110` — `Date.parse(updatedAt.replace(' ','T') + 'Z')` |

Two things found while verifying: bugs 3 and candidate 5 are **stated residuals in the code itself**
(`forge-engine-runtime.ts:214-221` says a losing QA worker can still record a disposition and that closing
it needs a shared transaction), so those stories close known gaps rather than discovering new ones. And
bug 2 turned up a **second ledger**: `forge_migration_execution` (this lane) and `schema_migration` (the
migration tool) hold the same fact in two homes.

## Bug candidates → batch 98 (verify first, then fix or record why not reachable)

| Astra # | Finding | Story |
|---|---|---|
| 5 | A losing QA worker can write a failure disposition | `ENG-FORGE-QA-RACE-01` |
| 10 | QA compares command counts, not identities | `ENG-FORGE-QA-RACE-01` |
| 6 | `Promise.all` rejects before sibling lanes finish | `ENG-FORGE-LANE-FAILURE-01` |
| 7 | Release/parity helpers close a shared pool | `ENG-FORGE-LANE-FAILURE-01` |
| 8 | Derived-model verification matches views by name, not schema | `ENG-FORGE-VERIFY-IDENTITY-01` |
| 9 | A stale refresh receipt can satisfy a later verification | `ENG-FORGE-VERIFY-IDENTITY-01` |

Each of those stories must **reproduce the case first** and record confirmed or refuted in its notes, then
either fix it or state why it is unreachable. A verify story that turns into a fix is fine; one that turns
into silence is not.

## Improvements (10) → batch 99

| Astra # | Item | Disposition |
|---|---|---|
| 1 | Crash recovery: completion + evidence + retry accounting in one recoverable operation | → `ENG-FORGE-CRASH-WINDOW-01` (same as bug 3) |
| 2 | Migration delivery ordered, resumable, safe to retry | → `ENG-FORGE-RELEASE-ORDER-01` + `ENG-FORGE-MIGRATION-REPLAY-01` |
| 3 | QA evidence bound to the exact code and acceptance criteria | COVERED: `ENG-FORGE-ACCEPTANCE-PROOF-01` (complete) + `ENG-FORGE-RECEIPT-COLUMNS-01` |
| 4 | Reserve shared-pool shutdown for process shutdown | → `ENG-FORGE-LANE-FAILURE-01` |
| 5 | Enforce ownership at every write, including late workers | COVERED: `ENG-FORGE-STATUS-WRITERS-01` + `ENG-FORGE-WRITER-MAP-01` + `ENG-FORGE-QA-RACE-01` |
| 6 | Verify referenced assertions actually ran, not just listed | FILED: `ENG-FORGE-ASSERTION-RAN-01` |
| 7 | Preserve stage, cause and recovery action in durable records | FILED: `ENG-FORGE-FAILURE-STAGE-01` |
| 8 | Test crashes between steps and real DB value formats | FILED: `ENG-FORGE-CRASH-TESTS-01` |
| 9 | Shorten historical commentary; keep current contracts distinct | FILED: `ENG-FORGE-COMMENT-DIET-01` (Low; must not erase a stated residual) |
| 10 | Operational metrics: completed stories, repair cost, human interventions | COVERED: `ENG-FORGE-RUN-SPEND-01` (complete) + `ENG-FORGE-BATCH-RECEIPT-01` + `ENG-FORGE-RECEIPT-COLUMNS-01` |

## Features (10) → batch 100

| Astra # | Item | Disposition |
|---|---|---|
| 1 | Resume preview | FILED: `ENG-FORGE-RESUME-PREVIEW-01` |
| 2 | Release reconciliation | FILED: `ENG-FORGE-RELEASE-RECONCILE-01` (Medium — motivated by tonight's devops-receipt hold) |
| 3 | Acceptance coverage explorer | FILED: `ENG-FORGE-ACCEPTANCE-EXPLORER-01` |
| 4 | Failure replay | COVERED: `ENG-FORGE-REPLAY-01` (sprint 95) |
| 5 | Preflight simulator | FILED: `ENG-FORGE-PREFLIGHT-01` |
| 6 | Cost-to-completion forecast | NOT FILED — blocked by `ENG-FORGE-RECEIPT-COLUMNS-01`: 595 of 935 runs record no cost, so a forecast would forecast a floor |
| 7 | Model performance report | NOT FILED — same blocker as 6 |
| 8 | Change-impact explorer | FILED: `ENG-FORGE-IMPACT-EXPLORER-01` |
| 9 | Stuck-story explanation | FILED: `ENG-FORGE-STUCK-EXPLAIN-01` |
| 10 | Completion receipt (one record joining acceptance, tests, published code, migrations, prod verification) | COVERED: `ENG-FORGE-BATCH-RECEIPT-01` + `ENG-FORGE-BLACKBOX-STORY-01` (sprint 95) |

## Astra's priority order, and where I departed from it

Astra: *"fix bugs 1–4, verify 5–10, then add release reconciliation and resume preview."* That is the order
these batches reflect. Two departures, stated:

1. `ENG-FORGE-CRASH-WINDOW-01` and `ENG-FORGE-QA-RACE-01` overlap at the QA disposition ordering. The crash
   story explicitly does not touch it, so both can land in either order without conflict.
2. Bugs 1 and 2 are the only ones I would call **release-blocking for the captain**, because he has been
   bitten by exactly that pair ("dev burning a prod release if the database does not go too"). If anything
   in these three batches is pulled forward, pull those two.

## Where I disagree with the review (to be read by the next reviewer, not to score points)

An external review sees a snapshot and usually only git. Three of these are places I would push back, and
one of them is a place I was wrong and Astra was right — recorded either way, because a review that is
never argued with is not being read.

**1. Bug 4 is real, and I doubted it wrongly.** `db/forge-engine-task-execution.ts:121` reads
`stale: !terminal && (!Number.isFinite(touched) || touched < staleBefore)`. The `!Number.isFinite` clause
means an unparseable timestamp AFFIRMATIVELY marks a non-terminal row stale — so the `Z`-append is not a
comparison that quietly fails, it is a green light to treat a live claim as reapable. Astra's wording
("marks nonterminal rows stale") was exact and my initial suspicion that NaN would read as *not* stale was
wrong. The story is updated to say so, and it now records that it REVERSES a deliberate clause: a reaper
must never reap what it cannot measure.

**2. Bug 2 severity, and the real hole.** A repeat requires the record write to fail *after* the SQL
succeeds, and schema migrations written as `begin; create table if not exists …; commit;` are harmless on
a second run — both migrations this repo shipped on 2026-09-17 are. So the systemic risk is not "migrations
repeat", it is **data migrations** (`insert`/`update`) replaying, where a second run changes data twice.
The story keeps the checksum-aware guard and WIDENS to require idempotency for a data migration, because
the guard alone cannot fix a migration that was never safe to re-run.

**3. Bug 3 framing.** "A crash between those operations leaves an advanced workflow without its result" is
literally true, but the state is LOUD rather than corrupt: the next role's gate reports the absent
deliverable and HOLDs (`forge-engine-runtime.ts:214-232` says so in as many words). So the cost is a human
plus a replay, not a wrong record — which is why the story exists (that cost is real and paid tonight) and
why it is not the same severity as bug 1.

**4. The review's test premise is wrong, and it matters.** Astra states that "the repository requires
production control-plane cleanup before tests, which I did not initiate for this review". It does not: the
engine suite runs as-is and was run repeatedly on 2026-09-17 — **761 tests, 758 pass, 0 fail, exit 0**, plus
`pnpm build`, `pnpm typecheck` and `pnpm lint` all green. So the grade is source-only by choice, not by
necessity, and the "substantial focused test coverage" he credits the repo with is in fact larger than he
assumed and directly runnable.

None of this weakens the four confirmed bugs. Astra found a release-ordering defect that has bitten the
captain before, a ledger/receipt ordering defect, a crash window the engine had only stated, and a
timestamp parse that disables the reaper's own safety — reading source alone. That is exactly what an
external reviewer is for, and the batch-98 stories carry his file:line evidence next to mine.
