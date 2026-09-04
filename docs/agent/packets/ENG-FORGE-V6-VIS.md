# ENG-FORGE-V6-VIS — One Truth, Three Lenses (Run Contract Visibility)

## Goal
Expose the V6 frozen Run contract (`106_forge_v6_run_contract`) identically across Portal Tech, Slack, and CLI so a human never misreads Neon truth vs local Git vs tracking-ref state. No behavior change to Forge routing, Assay, or publish.

## Depends on
Factory punch-list P0+P1 landed (explicit `builderFlashOverride`, single policy commit seam, blocked `pi-harness`, uniform readiness, no `CORE_CAPABILITIES`, fail-closed shared profiles, explicit workspace, memoized registry, pinned `deepseek-chat`, `deepseek-review` lineage).

## Scope
Read-only visibility over existing seams only: `db/forge-run.ts` (`getForgeRunExecutionStory`, `getForgeRunMachineEvidence`, `getForgeLeadRunRecord`), `db/storyboard.ts` (`getStoryboardStory`, `listStoryRuns`), `lib/worker-workspace` (`resolveApprovedBaseRef`, `listWorkerWorkspaces`). Swarm stays V7: `SPLIT:n` remains parsed but Hold-gated, no auto-prune on publish (orphan detector + manual `remove` only).

## Architect brief
1. Portal Tech story detail (`app/portal/storyboard/[id]/page.tsx` + `story-detail-sections.tsx`): add Run Contract panel — frozen `*_snapshot` vs live story diff, `packet_sha` stale badge (`Neon sha != Git sha` via `admitExecutableContract`/`sha256Text`), `base_commit_hash` + `origin/main` tracking state, Lead `run_phase`/`lead_decision`/`lead_split_count` timeline with reason, machine counters + `failure_code`. Reuse existing repository reads; no new queries, no new tables.
2. Slack (`agent-runtime/slack-notifier.ts` + `scripts/agent-work.ts` call sites): extend `ForgeSlackContext` with `packetShaStale`, `leadDecision`, `leadReason`, `publishPreview` (`publishable`|`conflict`|`no-candidate`). Same 4 events, same fail-open redacted contract, no new event types.
3. CLI (`scripts/workspace-cli.ts`): add `doctor` (wrapper sha match, `origin/main` resolve, orphan worktrees vs active `agent_work_item`, per-profile registry readiness) and `agent:work --preview-publish <story>` (dry-run `merge-base --is-ancestor` + `verified==candidate`, no push).

## Context refs
- db/forge-run.ts
- db/storyboard.ts
- lib/worker-workspace/provisioner.ts
- lib/worker-workspace/publish.ts
- agent-runtime/slack-notifier.ts
- scripts/agent-work.ts
- scripts/workspace-cli.ts
- app/portal/storyboard/[id]/page.tsx
- components/portal/storyboard/story-detail-sections.tsx

## Acceptance criteria
1. Story detail shows frozen-vs-live contract diff and stale-packet badge; no Forge behavior change.
2. Slack messages carry packet-stale + Lead + publish-preview facts; delivery stays fail-open and redacted.
3. `pnpm agent:workspace doctor` and `--preview-publish` report factually with zero mutations.
4. `SPLIT:n` still Hold-gates; no swarm, no auto-prune, no new queue.

## Preconditions
- V6 Run contract (106) stable on DEV and PROD; factory punch-list landed.

## Postconditions
- Portal, Slack, and CLI read identical Run truth; V6-ROLES can rename against visible evidence.

## Skills
planner, workflow

## Loop
Empty on first pass.

## Test mode
SCOPED only. Full regression forbidden.

## Assay commands
- `pnpm exec tsx --test agent-runtime/slack-notifier.test.ts workflow_app/tests/workspace-cli.test.ts`
- `pnpm exec tsx --test agent-runtime/run-machine-evidence.test.ts agent-runtime/forge-transition.test.ts`
