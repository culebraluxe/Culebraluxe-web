# ENG-FORGE-V5-03R — Durable Contract Snapshot and Scheduler Integrity

> Historical incident-repair work item. Its failed/Interrupted runs remain truthful evidence, but it is no longer a hard predecessor for the V5 architecture pack. Direct outer-harness recovery repairs on current `main` supersede this story as a release gate.

## Goal
Close the ENG-FORGE-V5-03 control-plane incident so an admitted Forge story can run unattended without depending on a freshly-pulled local Git packet, without losing completed Smith work when an Assay recipe is absent, and without silently building a successor from stale local `main`.

## Depends on
ENG-FORGE-V5-03 accepted, exact-candidate Assay passed, and candidate `f8de1b6c75f1902b0332dc8f007e651c67551afc` published to `origin/main`.

## Scope
Harden only the executable-story admission/snapshot seam, Smith→Assay repair seam, worker integration-base selection, and scheduler deployment diagnostics required by the incident review. Keep Forge serial. No MCP, plugins, sessions, subagents, swarm, Pro, general workflow redesign, or UI work.

The incident record is `docs/agent/incidents/2026-09-03-forge-v5-03-control-plane.md`.

## Architect brief
Implement the smallest durable repair that removes the four incident failure modes:

1. An admitted story must persist the executable packet facts needed later by Forge in Neon: `test_mode`, `assay_commands`, and `packet_sha`. Git remains planning/source history; Neon becomes sufficient executable truth after admission.
2. Smith launch must require architect brief, acceptance criteria, execution target, selected runtime readiness/capabilities, and Field readiness, but MUST NOT require an Assay plan.
3. Smith→Assay must fail closed when the Assay plan is absent while preserving the Smith candidate. Adding the plan later must allow Assay of that exact existing candidate; it must not rerun Smith merely to regenerate the candidate.
4. The scheduler remains only a wake timer. It must not pull/fetch/switch/reset/rebase/stash or authenticate to GitHub. Manual diagnostic execution must invoke the exact deployed wrapper used by launchd, and install/status must prove deployed-wrapper byte identity.
5. A new Smith workspace must use the accepted integration state, not a potentially stale local checked-out `main`. The default approved integration ref must be an integration-tracking ref whose resolved commit is checked against the expected admitted/published base where that fact is available. A stale/unresolvable base fails closed; never silently fall back to local HEAD/main.

Migration 104 is intentionally limited to three nullable text columns on `storyboard_story`: `test_mode`, `assay_commands`, and `packet_sha`. Do not widen the schema in this story.

## Acceptance criteria
1. `storyboard_story` exposes nullable `test_mode`, `assay_commands`, and `packet_sha`, and the Storyboard repository reads/maps them into `StoryPacketFields` used by Forge.
2. A single admission/materialization seam validates a Git packet and persists its executable snapshot before/promoted with executable Ready work; partial manual admission cannot create a launchable Smith envelope.
3. For an admitted story, durable Neon `test_mode`/`assay_commands` win over local Git fallback. A missing/stale local packet cannot erase a persisted executable contract.
4. Missing Assay commands do not reject Smith launch. After a real Smith candidate exists, missing Assay commands cause Hold / needs-Assay-plan with the candidate preserved and no Assay launched.
5. Supplying the missing durable Assay plan can resume by enqueueing Assay against the exact existing candidate without rerunning Smith. Wrong/missing candidate still fails closed.
6. Scheduler wrapper performs no Git remote operation or branch mutation. `agent:scheduler:run` executes the exact deployed wrapper path launchd uses. Install/status exposes a deterministic repo/deployed wrapper fingerprint and fails closed on mismatch.
7. Successor Smith work does not silently branch from stale local `main`. Default base selection uses the accepted integration tracking state (or an explicit admitted base commit); a mismatch/unresolvable expected base is a factual Hold/Error before model execution.
8. Existing exact-candidate Assay and non-force publication semantics remain unchanged.
9. Focused regression tests cover: durable packet precedence, Smith-without-Assay-plan, preserved-candidate Assay resume, deployed-wrapper mismatch, and stale integration-base refusal.
10. No Pro, MCP, plugins, sessions, subagents, swarm, force push, full regression, or unrelated schema/code expansion.

## Assay commands
- `node --test scripts/agent-scheduler.test.mjs`
- `pnpm exec tsx --test agent-runtime/execution-contract.test.ts agent-runtime/assay-plan.test.ts agent-runtime/orchestrate-apply.test.ts agent-runtime/candidate-assay-handoff.test.ts`
- `pnpm exec tsx --test agent-runtime/accepted-candidate-publish.test.ts lib/worker-workspace/accepted-base.test.ts`

## Test mode
SCOPED only. Full regression forbidden.

## Historical disposition
Preserve the existing Hold/Error/Interrupted evidence. Do not mark this story Complete retroactively and do not use it as a global pack gate. V5-04 through V5-11 may proceed independently from the accepted V5-03 baseline; only explicit `HARD:` edges may block descendants.
