# TypeSafe failure-triage pilot

An operator-invoked experiment: choose a recorded Forge failure, ask Jev for a cause,
then record the cause you confirmed. Scout integration is deferred until these results
show useful accuracy. This is not an automatic repair agent.

## Install and first use

Apply `FORGE-TYPESAFE-TRIAGE.patch` from the repository root with
`git apply --check /path/to/FORGE-TYPESAFE-TRIAGE.patch`, then
`git apply /path/to/FORGE-TYPESAFE-TRIAGE.patch`.
The patch was prepared against main `b59250576152c08491ef779e613951418c89a7d8`.
No new dependency or migration is required. Node's built-in fetch calls the TypeSafe HTTP API.

Add `TYPESAFE_API_KEY` to your existing `.env.local` (never commit the key).
Existing production database configuration is used by the repository's normal DB gateway.

```sh
# First run the required local verification; these tests never call TypeSafe or a DB.
pnpm forge:clean
node --import tsx --test legacy/workflow_app/tests/typesafe-failure-triage.test.ts
git diff --check

# List the latest 30 failed artifacts, optionally restricted to a story.
pnpm forge:triage list
pnpm forge:triage list YOUR-STORY-ID

# Replace ARTIFACT-UUID with an id from that list.
pnpm forge:triage preview ARTIFACT-UUID
pnpm forge:triage analyze ARTIFACT-UUID

# Replace TRIAGE-UUID with the id returned by analyze.
pnpm forge:triage review TRIAGE-UUID ENVIRONMENT_FAILURE "Starting the stopped DB service fixed the same check."
pnpm forge:triage report
```

`preview` shows the actual state and questions that `analyze` will send. It does not
call TypeSafe or write anything. `analyze` makes one request, with a 15-second timeout
and no automatic retries. Missing credentials or empty evidence stop before that call.
Successful analysis appends one `typesafe-failure-observation` to `forge_tool_artifact`.
Its source artifact ID, story/run/SHA, prompt version, input hash, actual model,
probabilities, confidence, evidence sufficiency, latency and token usage are retained.
The original verdict, workflow evidence, stage failure and engine routing are unchanged.

Eligible sources are failed/held/interrupted `qa-assay-evidence`, `architecture-security`
and `run-verdict` rows. No filesystem logs are read. A terse source may legitimately
produce UNKNOWN; the model cannot recover missing evidence.

## Review and decision

Use a confirmed cause, not agreement based solely on how plausible the model sounds.
Choose an existing class:

`MISSING_CONTEXT`, `BAD_IMPLEMENTATION`, `BAD_ARCHITECTURE`, `BAD_TOOL_CONTRACT`,
`ENVIRONMENT_FAILURE`, `MISSING_GUARDRAIL`, `WEAK_TEST`, `DEPENDENCY_FAILURE`,
`DEPLOYMENT_FAILURE`, `UNKNOWN`.

Each review appends an auditable row. Correct a review by issuing the same command
again with the corrected class and evidence; the latest review is used. The report
uses the latest observation per source for the pinned model and prompt version, up to
200 sources, so repeated analysis does not inflate the case count. Re-analyzing a
source requires reviewing the new observation; old labels are not silently carried over.

Agreement is computed only over reviewed cases; no labels returns null, not 100%.
The report also shows uncertainty, unreviewed cases, latency and input tokens for the
displayed evaluations. It is not a full billing report and excludes superseded calls.
Confidence below 0.8, sufficiency below 0.8, or UNKNOWN sets `needsReview`. These are
provisional display thresholds, not calibrated automation gates. Every result remains
advisory even when `needsReview` is false.

Start with whatever failures exist. Inspect confidently wrong answers and missing
evidence before expanding to Scout. A tiny or selectively reviewed sample cannot
establish production accuracy; the report deliberately makes the denominator visible.

## Data and failure behavior

Only allowlisted fields are sent: source kind, observed verdict, bounded summary,
failure code/detail, policy violations and failed command excerpts. Arbitrary artifact
JSON, environment variables, client records and full transcripts are not serialized.
Common credential/URL/email patterns are redacted before truncation. This is best-effort
redaction, not a proof that arbitrary source text contains no secret; use preview.
Raw provider error bodies are never printed or persisted.

Malformed responses, unknown classes, model mismatches, missing distributions and
invalid probabilities/usage are rejected without saving an observation. Transport or
storage errors propagate to the CLI seam, which awaits the existing `app_error` writer
and exits nonzero. If error capture itself fails, the CLI explicitly reports that too.
No fabricated classification, synthetic PASS or repair retry follows a provider failure.

## Verification status for this delivery

Application TypeScript check, separate TypeScript check including the new test file,
focused ESLint on all four TypeScript files, and `git diff --check` passed.

Focused unit cases are included for redaction/bounds, empty input, response validation,
uncertainty, immutable source preservation, storage/provider failure, timeout, repository
normalization, review linkage, report denominators and CLI preview.
Unit execution and live verification were not performed in the delivery workspace:
the required `forge:clean` prerequisite could not run because `.env.local` is absent.
No live TypeSafe accuracy or database write is claimed. Run the commands above locally.

API contracts: https://docs.typesafe.ai/api and https://docs.typesafe.ai/primitives/choice.
Model: `jev-1.13.0`; prompt: `forge-failure-triage-v1`. Re-evaluate when either changes.

---

## How this lands in Forge (Cline, 2026-09-18)

Pulled in and run on this machine before committing. Placement follows the house shape: the judgment in
`legacy/workflow_app/forge/typesafe-failure-triage.ts`, storage in `legacy/db/forge-typesafe-triage.ts` **through the
existing `recordToolArtifact`** (no new table), the operator CLI in `scripts/forge-triage.ts`, the fence
in `legacy/workflow_app/tests/typesafe-failure-triage.test.ts` (12/12, no network, no database). It reuses the
repository's own `FORGE_FAILURE_CLASSES`, so there is no second vocabulary to keep in step.

**Measured, two live calls** (~780 input tokens, 591–740 ms, ≈$0.00003 each):

- On a terse "17 failed tests" record it answered **UNKNOWN 0.80** with sufficiency **0.43** rather than
  blaming code — the prompt's discipline (*do not infer a defect because a test failed*) holding.
- On `CANDIDATE_MISMATCH` it hedged 0.45 / 0.37 / 0.15 with confidence **0.38**.
- The report showed `agreement: null` with 0 reviewed and the denominator visible.

### The one rule: a sensor, never an oracle

Nothing here decides anything. The useful output is the **shape of its confusion**, not its conclusion:

1. **Thin evidence** (`evidenceSufficiency` low) — our *record* is too weak to reason about. That is a
   recording bug on our side, actionable without believing the model at all.
2. **A class distribution that moved** between windows — a new failure mode appearing.
3. **A confident class no stored evidence could support** — literally a place to look.

Each cluster becomes a **story**, and the story is where the truth is established the usual way, with a
fence and a QA ruling. It is an intake source, never a decider.

### Graduation bar — written down before the data, so it cannot be moved later

Advisory → mainline requires **all three**: measured agreement on *evidenced* labels at least equal to
the existing classifier; **≥50** reviewed cases; and **calibrated sufficiency** (low sufficiency must
actually correlate with wrong answers). Until then it stays operator-invoked, and the API key is the
off-switch: unset it and Forge loses a report and nothing else.

### Known gaps — filed, not pretended

1. **Sources are too narrow.** Eligible sources are `qa-assay-evidence`, `architecture-security` and
   `run-verdict`: **28** not-passing rows exist. In the same window the engine produced **13 hold
   records** and **44 task rows with an error recorded**, and none of them are visible here — a worker
   crash on a constraint violation left an artifact that says *PASS*. The failures that cost hours are
   the ones this cannot see. `ENG-FORGE-ATLAS-SOURCES-01` extends it to `forge_hold_record` and
   `forge_engine_task_execution.last_error`.
2. **Labels, not the model, are the bottleneck.** A report over unreviewed cases is a demo, not a
   measurement. Label the cases whose cause is already **proven by its fixing commit**
   (`ENG-FORGE-ATLAS-LABELS-01`).
3. **Spend is invisible.** A judgment call is a vendor cost; it is not yet recorded as one, so it does
   not reach the sprint accounting. Migration 190's vocabulary (`vendor` | `widgets` | `none`) is
   waiting for it (`ENG-FORGE-ATLAS-SPEND-01`).
4. **Comparability.** `redactorVersion` is now stamped with every observation alongside `inputHash`, so
   a change to the redaction rules cannot silently rewrite what an old evaluation meant.

### Where it must never go

Engine routing, gates, permissions, concurrency, retry budgets, "did the tests pass", release
decisions. It is never cited as **evidence**, only as a pointer to where to look, and the hard gates
stay in code where they have always been.

