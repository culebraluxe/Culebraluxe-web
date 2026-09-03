# ENG-FORGE-V4-10C — Exact Candidate Assay Handoff

## Goal
Make Assay verification truthful and deterministic: every Assay lane for a code-changing Smith run must execute against the exact Smith candidate commit, and a failed scoped verification command must never normalize to Complete.

## Scope
Fix only the Smith → Assay handoff and Assay terminal normalization.

- When Smith produces a candidate commit, persist/resolve that exact commit as the required Assay base.
- Provision the Assay workspace from the Smith candidate commit, not from current `main`.
- If no candidate exists for a code-changing Smith run, do not launch Assay as though verification were possible; Hold with factual evidence.
- If any required Assay command fails, returns non-zero, is missing, or the candidate cannot be resolved, the Assay result must be Hold/failed verification — never Complete.
- A clean Assay must record which candidate SHA it verified.
- Accepted-candidate publication must only run after a clean Assay that verified the same candidate SHA being published.
- Preserve the Smith candidate branch/worktree on any handoff or verification failure.
- No schema changes unless absolutely required; prefer existing run/work-item evidence.
- No provider, model, Slack, swarm, or lane-order changes.

## Architect brief
V4-11 exposed a concrete false-positive path: Smith created candidate `549866555152c6f4bb55ffa9d45f19910ffab9f5`, but Assay workspace `626c756a-9bfd-46ad-9f6c-042de261f481` was provisioned from `main@7b14c6b...`. The first packet Assay command failed because `agent-runtime/slack-notifier.test.ts` did not exist in that checkout, yet the verifier work item/story normalized to Complete 100%.

The invariant is strict:

`Smith candidate C -> Assay workspace base C -> Assay evidence explicitly about C -> only then may C publish.`

Assay is an acceptance gate, not a generic lane completion. Infrastructure/provisioning failure or failed verification is completed work-item execution with a Hold/verification-failed outcome, not story success.

## Acceptance criteria
1. Smith candidate commit is the exact base/ref used to provision the following Assay workspace.
2. Assay evidence records the candidate SHA actually verified.
3. Missing/unresolvable candidate fails closed to Hold; Assay does not silently fall back to `main`.
4. Any required Assay command non-zero/missing-file failure produces Hold/verification-failed semantics, never Complete.
5. Accepted-candidate publish verifies Assay candidate SHA == publish candidate SHA before pushing.
6. Candidate branch/worktree remains preserved on any failure.
7. Existing V4-08 execution-contract, V4-09 harness-owned commit, V4-10 telemetry, and V4-10B non-force publish behavior remain unchanged.
8. Focused tests reproduce the V4-11 failure case and prove it cannot become Complete.

## Test mode
Scoped only. Do not run the full regression suite.

## Assay commands
- `pnpm exec tsx --test agent-runtime/candidate-assay-handoff.test.ts`
- `pnpm exec tsx --test agent-runtime/accepted-candidate-publish.test.ts agent-runtime/harness-owned-commit.test.ts agent-runtime/orchestrate.test.ts agent-runtime/orchestrate-apply.test.ts agent-runtime/repositories.assay.test.ts`

If a command fails, stop and report Hold/failure evidence. Do not report Complete.

## Skills
- TypeScript
- git/worktree/ref handling
- Forge lane orchestration
- Assay normalization
- fail-closed state machines

## Loop
Architect packet -> Smith -> exact candidate Assay -> publish to main only after clean matching-candidate evidence. No swarm, no provider change, no schema unless unavoidable.
