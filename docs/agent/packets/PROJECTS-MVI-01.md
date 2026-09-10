# PROJECTS-MVI-01 — Truthful service-backed Work Plan

## Story / Goal

Make `/portal/projects` load a truthful Work Plan through the canonical Project and WBS services. Completed work, hierarchy, status, order, and progress must reflect persisted WBS state. Runtime service failures must be visible and must never substitute the design fixture.

## Why this next

The current page calls the Catch-Up `listDue` query, which excludes completed rows while the Projects projection calculates progress from completed rows. This forces incorrect zero progress and can orphan an open child when its completed parent is absent.

## Canonical truth and ownership

- `ProjectService` owns Project reads.
- `WbsService` owns WBS reads.
- `wbs_item.sort_order` owns sibling order.
- Persisted Project/WBS statuses are authoritative.
- `ui/projects/service-projection.ts` maps service values into the MVI page model.
- The design fixture remains test/prototype input only and is never a runtime failure fallback.

## Scope

- Add a WBS service query for all project-bound WBS items and implement it in the SQL adapter.
- Load Projects and WBS items through service envelopes with the authenticated portal context.
- Project completed, doing, open, archived, and dismissed state truthfully into the UI model.
- Calculate progress from the complete plan, excluding dismissed work from the denominator without counting it as complete.
- Preserve descendants regardless of parent status.
- Honor `sort_order` before due date/id fallback.
- Make the Projects page show a captured, explicit read failure instead of fixture data.
- Add targeted service/projection tests.

## DO NOT

- Do not change PROD or DEV data.
- Do not add or apply a migration.
- Do not implement playbook identity, project creation, secondary tabs, inspector mutations, or unrelated visual changes.
- Do not special-case Casa Luar or any listing.
- Do not remove the obsolete `wbs_project` API in this story.
- Do not run the full regression suite.
- Do not push, merge, or deploy.

## Acceptance criteria

- A project with done and open WBS rows displays both and reports non-zero correct progress.
- An open child beneath a done parent remains visible beneath that parent.
- Siblings render by persisted `sort_order` with stable fallback ordering.
- `open`, `doing`, `done`, and `dismissed` remain distinguishable in the MVI.
- Actual Project status is not hardcoded to active.
- The runtime page obtains Projects/WBS data through `ProjectService.execute` and `WbsService.execute`.
- A service/read failure produces an explicit Projects unavailable state; fixture properties never appear as fallback.

## Targeted verification

- Targeted Project/WBS service tests.
- Targeted Projects service-projection tests.
- `git diff --check`.
- `pnpm exec tsc --noEmit` when dependencies are available.

## Handoff / commit expectations

Report exact files changed, commands/results, remaining PROJECTS-MVI-02 gaps, and working-tree status. Keep the candidate local; do not push or merge.

## Stop condition

Stop and report before any schema change, PROD/DEV mutation, playbook redesign, or ambiguous domain-ownership change.
