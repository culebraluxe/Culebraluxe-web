# CIL-06 — Lead PRE reads and writes the council verdict

## Why

`LEAD_DECISION: SOLO | SMITH | HOLD` already exists. A parallel prose
adjudicator is how we got untrustworthy QA. Lead should call council, then
emit the same machine line plus dissent/kill.

## Units

- **A** — `forge_lead_pre` subjectType, `subject_id = story id`.
- **B** — Lead PRE convenes only when the classify gate in CIL-07 would
  allow it. Otherwise `refuse` with reason `cheap-default`.
- **C** — Extend `parseLeadDecision` to accept optional
  `LEAD_DISSENT` / `LEAD_KILL` / `LEAD_NEXT` **or** load them from
  `getBySubject`. One writer remains `council_verdict`.
- **D** — HOLD maps to a refused or kill-triggered session, not a second
  status enum.
- **E** — Tests around `agent-runtime/lead-decision.ts` only. No topology XML
  edit.

## Acceptance

- A Lead PRE HOLD without inventing SPLIT still parses.
- When a council verdict exists for the story, Lead reason cites
  `session_id`, not a restated essay.
- Smith never calls council.

## Not this story

- Architect duo gate is CIL-07.
- Assay checkpoint is CIL-08.
- No new lane in `lanes.ts`.
