# CIL-03 — CouncilService kernel

## Why

App and Forge need one envelope surface. Routes and lanes must not speak SQL.

## Units

- **A** — Land `services/council/*` from the pack. Align imports with
  `services/core` as it exists on HEAD.
- **B** — Operations: convene, recordVerdict, checkpoint, get, getBySubject,
  verifyVendor. `full` mode fails. Dissent/reasons/kill bounds enforced.
- **C** — `decideRefuse` covers lookup, bless-my-answer, last-contact/identity.
- **D** — `NullModelPort` throws `COUNCIL_MODEL_UNBOUND` if someone tries a live
  deliberation before CIL-05.
- **E** — Targeted tests for refuse, inflight reuse, verdict bounds, frozen
  outcome. No FULL suite.

## Acceptance

- `convene` on an inflight subject returns the existing session.
- `recordVerdict` without dissent fails `DISSENT_REQUIRED`.
- `checkpoint` twice fails `OUTCOME_FROZEN`.
- Vendor drift fails `COUNCIL_VENDOR_DRIFT` before insert.
- Invariants() name "not a Forge lane" and "one verdict per session".

## Not this story

- Not registered in `composeCoreServices` yet (CIL-04).
- No DeepSeek/OpenCode port.
- No Lead parser changes.
