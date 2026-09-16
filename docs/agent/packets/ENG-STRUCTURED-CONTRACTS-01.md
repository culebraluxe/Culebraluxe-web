# ENG-STRUCTURED-CONTRACTS-01 — the worker says "I am alive" in a row, and no contract is JSON

## Goal

Two changes that are one change: **(1) liveness is a database row, not a log file**, and **(2) no role
contract travels as JSON**. A model that cannot meet the field contract is not routed to the seat — that is a
tooling decision, not a prompt scolding.

## Why

`pnpm forge:doctor` currently answers "is the worker alive?" by reading and parsing
`~/Library/Logs/CulebraLuxe/agent-worker.invocations.log` line by line (`start:`, `stop:`, `last failure:`).
That is a parser over prose, and it exists only because the worker never recorded the fact anywhere queryable
— so the one question an operator asks before every run costs a text parse over a growing file.

The same shape is everywhere in the contracts: `FORGE_ARCHITECT_HANDOFF:`, `FORGE_FINDINGS_JSON:` and
`FORGE_EVIDENCE_JSON:` make a model's punctuation authoritative. Two weeks of that produced a long trail of
seam failures, and the freshest one is the cleanest example: the advertised Architect schema was missing a
single closing brace (`"hint":"SAME_UNIT"]}` instead of `"hint":"SAME_UNIT"}]}`), so every Architect prompt
carried a schema the documented parser could not read. **That one was ours, not a model's** — which is the
argument: if there is no JSON contract, neither we nor they can malform it.

The direction is already established in one place: `docs/agent/MEMORY.md:71` records that the Architect
handoff moved to `forge_role_finding` rows and *"rows win; the reply parser is only the fallback."* A fallback
that can disagree with the rows is a **second verdict**, and second verdicts have already cost this system
nights.

## Scope

- `db/migrations/<next>_worker_heartbeat.sql` (new): `forge_worker_heartbeat` — `agent_id`, `pass`, `story_id`,
  `node`, `model`, `outcome`, `started_at`, `ended_at`, `detail`. Append-only; a failed pass is a row too.
- `db/forge-heartbeat.ts` (new): writer/reader at the repository boundary, normalizing driver values (ISO
  timestamps, numbers) per `AGENTS.md`.
- `scripts/agent-worker-once.sh`: write a heartbeat at pass start and at pass end (including `idle`, failure
  and exit code). No JSON in the worker's own path.
- `scripts/forge-doctor.ts` + `workflow_app/forge/forge-doctor-report.ts`: worker liveness is a **query**. The
  log becomes history, read only when the row is absent, and the report says which source answered.
- `workflow_app/forge/agents/architect-handoff.ts` + the typed-evidence reader: once rows are authoritative,
  **delete the JSON reply fallbacks** rather than keeping a parser that can outvote the rows.
- `lib/forge-kind.ts` + the policy→model map: a policy may name only a **tokens-runnable, headless** model.
  `judgment` currently resolves to an interactive-only model and must not fill a seat.

## Do not touch

- Grok's `judgment-lab` seat (captain-only, letters only) and anything that would wire it into the worker.
- The frozen-proof Assay path, the QA candidate pin, or the release record.
- `forge_role_finding` and the rows that already replaced the findings chat-JSON.

## Assay (SCOPED)

- `node --import tsx --test workflow_app/tests/forge-doctor-report.test.ts`
- `node --import tsx --test workflow_app/tests/forge-heartbeat.test.ts` (new) — the heartbeat's write/read
  shape, an idle pass, a failed pass, and that a missing row is reported as *unknown*, never as healthy.
- `node --import tsx --test workflow_app/tests/forge-kind-routing.test.ts`

Test mode: **SCOPED**. No FULL regression for this story.

## The rule we are enforcing (captain, 2026-09-16)

**Write whatever you want; do not pass it that way.** A lane may narrate in prose, use braces, write an essay —
none of that is anyone's business, and nothing here forbids it. What **crosses a seam** must be formatted to
fit Neon: a named field in a row, or a token from a closed set. Clean the kitchen after you use it — the next
agent must never be killed by a parse error somebody left behind.

Enforcement is **structural, not rhetorical**:

- the writer — our code, never the model's free text — validates before insert and refuses with the **field
  name**, so a bad value is a gate failure naming a field, never a lane failure blamed on a model;
- silent tolerance is itself the defect: a value that cannot be represented is refused, not coerced and not
  dropped;
- free text may remain in the transcript as evidence; it is never the transport.

## Acceptance criteria

1. The doctor answers worker liveness **from Neon**; a test proves the happy path does not read the log, and a
   missing heartbeat row reports *unknown* rather than healthy.
2. Every pass writes exactly one heartbeat row, including idle and failed passes; a missing row is not evidence
   of a clean pass.
3. No role is *required* to emit JSON: with the reply parsers deleted, a lane still routes from its fields —
   proved by a test that removes the parser input and asserts the row-based path still decides.
4. No parser may overrule a row. If the two ever disagree, the row wins and the disagreement is recorded.
5. Every policy in the policy→model map names a model that runs headless with tokens; a policy pointing at an
   interactive-only model fails a test rather than a run.
6. A guard test fails if a new JSON contract is introduced into a role prompt or a lane's handoff.

## Out of scope (stop if you start these)

- Choosing or grading models, or any claim about which model is smarter.
- A second heartbeat mechanism, a metrics store, or a dashboard.
- Rewriting the packets, the manifest, or the release record — those are already markdown, not JSON.

## Sign-off

- Builder reports the exact files changed and the Assay commands above.
- Reviewer checks: can a log parse still decide liveness; can a parser outvote a row; does any policy name a
  model that cannot run headless.
