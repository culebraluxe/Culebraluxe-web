# ENG-FORGE-V6-ROLES — Six-Role Topology and Renames

## Goal
Lock Forge to six human-readable roles (Scout → Architect → Lead → Smith 1..N → QA → DEV_OPS), rename Assay → QA and publish → DEV_OPS responsibility, and preserve the exact-candidate invariant as a QA operation. No behavior change to Story Run truth.

## Scope
Topology and vocabulary only. Rename role labels and lane docs; keep `storyboard_story_run` architecture, Neon truth, worktrees, candidate SHA lineage, and non-force publish mechanics unchanged. Inspector becomes a QA capability (independent-review rule), not a seventh agent. Archive/night remain capabilities/grades, not roster roles. No CrewAI ingest, no second orchestrator, no swarm activation.

## Architect brief
1. Roster is exactly: Scout (what is going on?), Architect (what should we build/change?), Lead (how do we get this story done?), Smith 1..N (can I build it correctly?), QA (is it correct and ready to ship?), DEV_OPS (can I safely get it into production and prove it?).
2. Rename Assay → QA at role/label level only. Preserve `Assay verified candidate SHA == publish candidate SHA` as `QA → Candidate Assay` evidence operation with PASS/FAIL math unchanged.
3. Rename publish responsibility → DEV_OPS: Git/mainline, migrations, environment verification, production health, and durable deployment evidence. No force-push; conflict still fails closed to Hold.
4. QA contains: contract verification, acceptance criteria, targeted tests, regression selection, security/policy checks, integration verification, and Candidate Assay operation.
5. Keep serial execution and single-active lock. SPLIT:n stays parsed but Hold-gated for future N-Smith.

## Context refs
- agent-runtime/lanes.ts
- agent-runtime/team.ts
- agent-runtime/forge-transition.ts
- agent-runtime/assay-evidence.ts
- agent-runtime/accepted-candidate-publish.ts
- docs/agent/packets/ENG-FORGE-V5-11.md

## Acceptance criteria
1. Six roles documented with one-question definitions; no seventh roster agent added.
2. Assay → QA rename complete in labels/docs without weakening exact-SHA gate.
3. Publish → DEV_OPS responsibility defined (Git, migrations, env verify, health, evidence) with non-force invariant intact.
4. Inspector preserved as QA independent-review capability (lineage separation rule intact).
5. No new queue, no orchestrator split truth, no swarm.

## Preconditions
- ENG-FORGE-V6-VIS accepted or in progress; V6 Run contract stable.

## Postconditions
- Follow-up QAOPS/DEVOPS1 stories can implement renames against this locked topology.

## Skills
planner, workflow

## Loop
Empty on first pass.

## Test mode
SCOPED only; use exact touched tests when available.

## Assay commands
- `pnpm exec tsx --test agent-runtime/team.test.ts agent-runtime/lane-policy.test.ts`
- `pnpm exec tsx --test agent-runtime/forge-transition.test.ts agent-runtime/assay-evidence-v6.test.ts`
