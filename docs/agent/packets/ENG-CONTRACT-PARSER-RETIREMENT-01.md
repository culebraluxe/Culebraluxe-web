# ENG-CONTRACT-PARSER-RETIREMENT-01 — retire the JSON reply parsers, so no parser can outvote a row

## Goal

Delete the JSON reply parsers that still stand beside the rows: the Architect handoff reply line, the findings
JSON and the evidence JSON. A reply may still *contain* anything the model likes — it simply is no longer read
as authority. This is the **removal** half of the contracts work; the heartbeat and the additive field plumbing
live in `ENG-STRUCTURED-CONTRACTS-01`.

## Why

`docs/agent/MEMORY.md:71` records that the Architect handoff already moved to `forge_role_finding` rows and that
*"rows win; the reply parser is only the fallback."* A fallback that can disagree with the rows is a **second
writer** — and `AGENTS.md` (Never) now forbids two sources for one fact outright: a disagreement is a REFUSAL
naming both, never a resolution that picks a winner.

Two weeks of JSON-contract failures argue the same thing from the other side, and the sharpest example was ours:
the advertised Architect schema was missing the brace that closes the finding object (`"hint":"SAME_UNIT"]}`
instead of `"hint":"SAME_UNIT"}]}`), so every Architect prompt carried a contract the documented parser could
not read. Nothing has to be malformed if there is no JSON contract to malform.

## Scope

- `legacy/workflow_app/forge/agents/architect-handoff.ts` — the reply parser becomes a **reader of last resort for
  diagnostics only**: it may log what a reply said, and it may not decide anything.
- The typed-evidence reader (`FORGE_EVIDENCE_JSON`) and the findings marker (`FORGE_FINDINGS_JSON`): same shape —
  values come from the fields that were written, never from the text that was typed.
- A guard test that fails if a role prompt or a lane handoff **requires** JSON again.

## Do not touch

- `forge_role_finding` and the rows that already replaced the findings chat-JSON (they are the destination, not
  the subject).
- The mediator (`lib/field-mediator.ts`), the release record, or the QA candidate pin.

## Assay (SCOPED)

- `node --import tsx --test legacy/workflow_app/tests/forge-architect-role.test.ts`
- `node --import tsx --test legacy/workflow_app/tests/forge-qa-seam.test.ts`

Test mode: **SCOPED**. No FULL regression for this story.

## Acceptance criteria

1. With the parser's input removed or garbled, every lane still decides from its written fields — proved by a
   test, not by inspection.
2. When a parser and the rows disagree, the row stands **and the disagreement is recorded as a REFUSAL naming
   both** — no winner is picked and nothing is silently preferred. Fixture: row says `HOLD`, parser says
   `SAME_UNIT` / `SMITH` → `HOLD` persists, both sources are recorded, the lane HOLDs.
3. No role prompt or handoff requires JSON: a guard test fails if one is reintroduced.
4. Free text stays in the transcript as evidence; it is never the transport.

## Out of scope (stop if you start these)

- Replacing the parsers with a smarter parser, a tolerant parser, or a second adapter.
- The heartbeat row and the additive field plumbing — those are `ENG-STRUCTURED-CONTRACTS-01`.

## Sign-off

- Builder reports the exact files changed and the Assay commands above.
- Reviewer checks: can a parser still outvote a row; does any lane now need JSON to function.