# Forge V5-03 Control-Plane Incident — 2026-09-03

## Summary

ENG-FORGE-V5-03 was delayed by a chain of control-plane defects. No unverified code was published and no candidate was lost, but the unattended machine stopped repeatedly and required manual intervention.

This was not one defect. It was three coupled failures plus one operator-process gap.

## Failure chain

1. **Split executable contract**
   - Story intent/brief/acceptance lived durably in Neon.
   - `## Test mode` and `## Assay commands` lived only in the Git packet/local checkout.
   - Forge merged Neon + local Git at execution time.
   - A Ready story could therefore be valid in Neon while the worker saw an incomplete executable packet because its local checkout was stale.

2. **Assay requirement enforced too early**
   - The Smith launch gate originally required an Assay plan before Smith could execute.
   - Missing verification instructions therefore converted a recoverable handoff omission into a Smith launch Error.
   - Correct semantic boundary: Smith needs brief + acceptance + target + runtime readiness. Assay commands are required only at Smith→Assay handoff.

3. **Scheduler gained a second failure plane**
   - A mitigation made launchd run `git pull --ff-only origin main` before Forge.
   - Under launchd, branch probing returned an empty branch and then remote Git sync failed even though interactive Git worked.
   - The scheduler should not authenticate to GitHub or mutate repository state. It is only a wake timer.

4. **Diagnostic path did not equal scheduled path**
   - launchd executed the deployed wrapper under `~/Library/Application Support/CulebraLuxe/`.
   - `pnpm agent:scheduler:run` executed the repo copy instead.
   - install/status had no byte fingerprint proving the deployed copy matched the repo copy.
   - Old invocation log lines could look like current failures even after the deployed wrapper had changed.

## What protected the system

The fail-closed execution contract prevented Smith/Assay from fabricating missing facts. Exact-candidate Assay remained intact. V5-03 ultimately produced candidate `f8de1b6c75f1902b0332dc8f007e651c67551afc`, Assay ran against that exact SHA, all required tests passed, and the candidate was published non-force to `origin/main`.

## Permanent invariants

1. **Neon is executable truth.** A story admitted for execution must carry a durable snapshot of the executable packet, including test mode, Assay commands, and packet provenance.
2. **Git is planning/source history, not a runtime dependency.** An already-admitted story must not require a fresh GitHub pull to discover its execution contract.
3. **Smith can finish before Assay is specified.** Missing Assay instructions after a valid candidate means Hold / needs Assay plan; preserve the candidate and never rerun Smith just to obtain verification instructions.
4. **Assay remains fail-closed.** No Assay commands means no Assay. Wrong/missing candidate means no Assay. No verified candidate means no publish.
5. **Scheduler is only a wake timer.** No branch switching, pull, fetch, rebase, reset, stash, or remote authentication in the launchd wrapper.
6. **Diagnostic and scheduled execution are identical.** `agent:scheduler:run` must execute the exact deployed wrapper used by launchd.
7. **Deployment integrity is visible.** install/status must prove repo/deployed wrapper byte equality with a fingerprint.
8. **Successor work bases on accepted integration state.** A new Smith must not silently branch from a stale local `main`; the approved base must resolve to the accepted integration ref/commit and fail closed when unavailable.
9. **No partial manual admission.** Story admission must validate and persist the executable snapshot before creating/promoting executable work.
10. **Evidence disagreement repairs state, never success evidence.** Complete, Hold, candidate, Assay, and publish state remain derivable from durable evidence.

## Remediation

### Already applied
- Removed the pre-Smith Assay-plan requirement from the Smith execution contract.
- Smith→Assay remains the Assay-plan enforcement boundary.
- Candidate-preserving Hold behavior exists when the Assay plan is missing after Smith.
- Removed Git sync/branch probing from the launchd wrapper.
- Hardened scheduler diagnostics so `agent:scheduler:run` executes the deployed wrapper.
- Added SHA-256 repo/deployed wrapper integrity reporting and fail-closed install/run checks.
- Added focused scheduler integrity tests.

### ENG-FORGE-V5-03R closure work
- Persist `test_mode`, `assay_commands`, and `packet_sha` with the executable story snapshot in Neon.
- Add one admission/materialization seam that validates and persists the packet before Ready work is created.
- Make Smith workspaces use the accepted integration base rather than a potentially stale local checked-out `main` ref.
- Prove candidate-preserving Assay resume after a missing-plan Hold.
- Add focused regression tests for every failure in this incident.

## Exit criterion

V5-04 may not start until ENG-FORGE-V5-03R is accepted and published. The incident is closed only when a Ready story can execute from durable Neon contract facts, the scheduler can run without GitHub access, the deployed wrapper identity is provable, and a missing Assay plan cannot discard or rerun completed Smith work.
