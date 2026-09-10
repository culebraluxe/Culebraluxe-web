# PROJECTS-WORKSPACE-08 — Project Timeline projection (ARCHITECT CONTRACT)

Lane: architect. Design truth only. No implementation, no commit, no DEV/PROD write.

## Loop
intent: grow
parent_run: PROJECTS-WORKSPACE-07 (work-item domain actions and linked-record navigation)
loop: 1/3

## Goal (one line)

Project a selected project's real WBS work items into a deterministic Timeline — six mutually
exclusive date/state buckets (completed, current, upcoming, overdue, dismissed, undated), hierarchy
preserved, milestones marked, selection synchronized with tree/Work Plan/inspector, distinct
empty/unlinked/failure states, and portal-timezone date-only semantics.

## Non-goals (explicit)

- No schema change, no migration, no DDL. `wbs_item.due_at` already exists; there is no `start_at`
  and no timeline table.
- No new route and no new service domain; the timeline path performs **no DB read** — it projects
  the already-loaded `ProjectPlan.workNodes`.
- No change to Pane 1 tree, Pane 3 inspector fields, Calendar/Documents/Activity/Financials views,
  the view rail, the three-pane geometry, or the page controller's existing intents.
- No change to the inspector due-date **write** path (browser-local noon) — that is WS-06 scope
  (Risks 1).
- No second selection store: reuse `projects.selectNode` / `model.selectedNodeId`.
- No Gantt bars, drag/resize, dependency arrows, or start/end spans — "useful hierarchy" means
  indentation, not a chart engine.
- No listing special-casing; no full regression (SCOPED only); no push/merge/deploy; no DEV/PROD
  schema or business-data write.

## Verified starting evidence (grounding)

- **Timeline is a declared view but unrendered.** `ProjectWorkspaceView` includes `"timeline"`
  (`ui/projects/model.ts:125`); `VIEW_LABEL.timeline = "Timeline"`
  (`components/portal/projects-workspace.tsx:121`). Pane 2's view switch has branches for
  `work-plan`/`calendar`/`documents`/`activity` only, so `timeline` falls through to
  `<ProjectionState view="timeline">` and renders "Timeline is not available in this workspace yet."
  (`components/portal/projects-workspace.tsx:658-668`, `:604`).
- **Work items already carry real dates.** `WbsItem.dueAt` (`services/wbs/types.ts:19`);
  `wbs_item.due_at timestamptz` (`db/migrations/124_wbs_project_item.sql`); projected to
  `ProjectWorkNode.dueAt` + `dueLabel` (`ui/projects/service-projection.ts:156-157`, `:99-104`).
- **Hierarchy is already nested.** `ProjectWorkNode.children` is built recursively in `attach`
  (`ui/projects/service-projection.ts:147-167`); top-level = items with `parentId == null` (`:233`).
- **Milestones exist as a node type.** `ProjectWorkNodeType` includes `"milestone"`
  (`ui/projects/model.ts:17-27`); the fixture uses `type: "milestone"` (`ui/projects/source.ts:219`).
- **Status vocabulary.** `ProjectWorkStatus` = complete|waiting|in-progress|not-started|blocked|
  dismissed (`ui/projects/model.ts:15`), mapped from WBS status (`ui/projects/service-projection.ts:59-64`).
- **Selection is centralized.** `model.selectedNodeId`; intents `projects.selectNode` /
  `projects.selectProject` (`ui/projects/model.ts:159-167`,
  `ui/projects/projects-controller.ts:112-138`). Work Plan already dispatches `onSelectNode`
  (`components/portal/projects-workspace.tsx:424-465`); Pane 3 resolves the selected node via
  `findWorkNode` (`:169-180`, `:852`). NOTE: `selectProject`/`selectPole`/`selectDomain` reset
  `activeView: "work-plan"` (`projects-controller.ts:73,108,128`) — the Timeline must dispatch only
  `selectNode` so it stays on the Timeline view.
- **Operating-timezone contract exists.** `PORTAL_TIME_ZONE_OFFSET = '-04:00'` (Puerto Rico, no DST)
  and `toPortalInstant` (`lib/portal-time.ts:8,33`). Neither `lib/portal-time.ts` nor
  `lib/portal-write-error.ts` imports `server-only`, so it is safe to import into the `"use client"`
  component (`components/portal/projects-workspace.tsx:1`).
- **Discriminated states already exist.** `ProjectsWorkspaceStatus` = ready|empty|unauthorized|
  failure (`ui/projects/model.ts:104-115`); `ProjectionState` renders distinct `unlinked`/`empty`/
  default copy (`components/portal/projects-workspace.tsx:588-613`).
- **Precedent** for a pure React-free projection module + its own test: `ui/projects/secondary-projection.ts`
  + `testv2/projects-secondary-projection.test.ts`.
- **WS-06 / WS-07 are NOT present in this base** (checkout at origin/main@e154279): no
  `ui/projects/inspector-form.ts`, no `ui/projects/work-actions.ts`, no
  `testv2/projects-workspace-06-inspector.test.ts`. Pane 3 is still the pre-WS-06 free-text
  inspector (`components/portal/projects-workspace.tsx:692-813`). WS-08 must not import or assume
  those modules.

## Touched surfaces

Files
- `ui/projects/timeline-projection.ts` (NEW, React-free, no `server-only`, no I/O, no `Date.now()`):
  bucket types, `toPortalDateOnly`, `buildProjectTimeline(project, today)`, `timelineState`.
- `ui/projects/index.ts` — export the timeline module types + functions.
- `components/portal/projects-workspace.tsx` — add `ProjectTimeline`, wire the Pane 2
  `activeView === "timeline"` branch, thread `selectedNodeId` + `onSelectNode`; compute `today` once
  via portal date-only. All other views unchanged.
- `lib/portal-time.ts` — add pure `toPortalDateOnly(instant: string): string | null` reusing
  `PORTAL_TIME_ZONE_OFFSET`. No behavior change to `toPortalInstant`.

Tables
- None read directly by the timeline (projection runs over `ProjectPlan.workNodes`, already loaded
  from `wbs_item`). No DDL, no new query.

Routes
- `/portal/projects` only. No new route.

Tests
- `testv2/projects-workspace-08-timeline.test.ts` (NEW — story-owned, red→green).
- Existing, must stay green: `testv2/projects-secondary-projection.test.ts`,
  `testv2/projects-service-projection.test.ts`, `testv2/projects-workspace-01-readmodel.test.ts`.

## Contract

### C1 — Pure projection module (deterministic, injected "today")

`ui/projects/timeline-projection.ts` — no I/O, no clock, no randomness, no React.

```ts
export type ProjectTimelineBucketKey =
  | 'completed' | 'current' | 'upcoming' | 'overdue' | 'dismissed' | 'undated'

export type ProjectTimelineItem = {
  id: string
  title: string
  type: ProjectWorkNodeType
  status: ProjectWorkStatus
  dueAt: string | null          // original instant, unchanged
  dueDate: string | null        // portal date-only 'YYYY-MM-DD' or null
  isMilestone: boolean
  depth: number                 // 0-based within the bucket forest
  children: ProjectTimelineItem[]
}

export type ProjectTimelineGroup = {
  key: ProjectTimelineBucketKey
  label: string                 // fixed copy per key
  items: ProjectTimelineItem[]  // roots of the bucket's forest, stable order
}

export type ProjectTimelineState = 'ready' | 'empty' | 'unlinked' | 'failure'

export type ProjectTimeline = {
  state: ProjectTimelineState
  groups: ProjectTimelineGroup[]  // always the six keys, stable order
  total: number                   // nodes considered
  dated: number
  undated: number
}

export function toPortalDateOnly(instant: string | null | undefined): string | null
export function buildProjectTimeline(project: ProjectPlan | null, today: string): ProjectTimeline
```

- `today` is REQUIRED, portal date-only `'YYYY-MM-DD'`; the module never calls `Date.now()` /
  `new Date()` for "now".
- `buildProjectTimeline` traverses the FULL `workNodes` tree (all descendants), never just top-level.
- Invariant: pure — same `(project, today)` → deep-equal output; idempotent; no dependence on object
  identity.

### C2 — Date-only semantics (portal timezone)

- `toPortalDateOnly` converts a stored instant (`wbs_item.due_at`, absolute ISO UTC) to the Puerto
  Rico calendar date using `PORTAL_TIME_ZONE_OFFSET` (`-04:00`, no DST). Invalid or missing → `null`.
- All bucketing compares `'YYYY-MM-DD'` strings lexicographically — never `Date` instants, never the
  browser's local timezone.
  - `overdue`: `dueDate < today`
  - `current`: `dueDate === today`
  - `upcoming`: `dueDate > today`
- Invariant: an instant's bucket does not depend on the rendering browser's timezone or the machine
  clock.

### C3 — Exactly-once bucket precedence

Assignment is a single ordered decision per node:

1. `dismissed` — `status === 'dismissed'` (regardless of date)
2. `completed` — `status === 'complete'` (regardless of date)
3. dated bucket by C2 (`overdue` | `current` | `upcoming`)
4. `undated` — no valid `dueDate`

- Invariant: every node lands in exactly one bucket; flattening all groups yields each node once and
  the count equals `total`. `dismissed`/`completed` win over date, so a completed past-due item is
  `completed`, not `overdue`.

### C4 — Hierarchy preservation

- Walk the project tree depth-first in canonical child order (the order `service-projection`'s
  `compareItems` already produced).
- A node is placed in its own bucket. A child whose bucket equals its parent's is nested under it; a
  child whose bucket differs attaches to its nearest same-bucket ancestor if one exists, otherwise
  becomes a bucket root. `depth` is the within-bucket depth (0 at bucket roots).
- Invariant: within a bucket, every original ancestor/descendant pair that shares the bucket keeps
  that relationship; no node is dropped and no synthetic node is invented; sibling order is stable.

### C5 — Milestones

- `isMilestone = node.type === 'milestone'`.
- Milestones are never moved to a special bucket; they render with a distinct `Flag` marker in their
  date/state bucket. Invariant: a milestone's bucket follows C3 exactly like any other node.

### C6 — Distinct states (empty / unlinked / failure)

- `state`:
  - `unlinked` — `project == null` (no project selected). Copy: "Select a project to see its timeline."
  - `empty` — project selected, `workNodes` empty (no work items at all). Copy: "This project has no work items yet."
  - `ready` — at least one node. Even when every node is undated, state is `ready` (the `undated`
    bucket is explicit, never "empty").
  - `failure` — a projection/read error surfaced by the caller; the pure function returns
    `ready`/`empty`/`unlinked`, and the component maps a caught throw/load error to `failure`
    (routed through the existing error-capture seam; never a 500, never fabricated data).
- Invariant: the four states are pairwise distinct in state value and copy; a test asserts the
  message strings differ.

### C7 — Selection synchronization (single source of truth)

- `ProjectTimeline` receives `selectedNodeId` and `onSelectNode`.
- Clicking a timeline item dispatches the existing `projects.selectNode` intent via `onSelectNode`;
  it holds NO local selection state. The selected item is highlighted from `model.selectedNodeId`.
- The same `selectedNodeId` drives Work Plan highlighting and Pane 3's `findWorkNode` inspector, so
  selecting in tree / Work Plan / Timeline highlights the other two and opens the inspector.
- Selecting a node from the Timeline dispatches ONLY `selectNode` (never `selectProject`/
  `selectPole`/`selectDomain`, which reset `activeView` to `work-plan`).
- Invariant: no second selection store; Timeline selection survives switching to Work Plan and back.

### C8 — Rendering

- Pane 2 adds `activeView === "timeline" ? <ProjectTimeline .../>` before the `ProjectionState`
  fallback; all other branches unchanged.
- Groups render in fixed order completed → current → upcoming → overdue → dismissed → undated.
  Empty groups may be omitted from the DOM, but the `undated` group must render whenever undated
  items exist. Each group header shows its label and count.
- Each item is a `<button type="button">` with `aria-current` (or `aria-pressed`) for the selected
  node, indentation derived from `depth`, a `Flag` marker when `isMilestone`, and the existing
  `StatusIcon`/status-colour helpers for state.
- The `ProjectionState` fallback for `timeline` is removed; the C6 states render distinct copy.

## Acceptance (verifiable checks)

| # | Check | Assay command |
|---|-------|---------------|
| AC1 | Every dated item appears exactly once in the correct bucket; flattened counts equal `total` | `pnpm exec tsx --test testv2/projects-workspace-08-timeline.test.ts` |
| AC2 | Date-only bucketing uses portal TZ: an instant whose portal date differs from browser-local date lands in the portal-date bucket | `pnpm exec tsx --test testv2/projects-workspace-08-timeline.test.ts` |
| AC3 | Undated work is explicit: the undated bucket carries exactly the undated ids; no dated id leaks in | `pnpm exec tsx --test testv2/projects-workspace-08-timeline.test.ts` |
| AC4 | Hierarchy preserved within a bucket; a cross-bucket child promotes to nearest same-bucket ancestor | `pnpm exec tsx --test testv2/projects-workspace-08-timeline.test.ts` |
| AC5 | Milestones flagged (`isMilestone`) and bucketed by the same C3 rules | `pnpm exec tsx --test testv2/projects-workspace-08-timeline.test.ts` |
| AC6 | Selection sync: source-scan proves Timeline dispatches `onSelectNode`/`selectNode` and does not call `selectProject`; highlight derives from `selectedNodeId` | `pnpm exec tsx --test testv2/projects-workspace-08-timeline.test.ts` |
| AC7 | empty / unlinked / failure copy are pairwise distinct | `pnpm exec tsx --test testv2/projects-workspace-08-timeline.test.ts` |
| AC8 | Determinism: two calls with the same inputs deep-equal; module contains no `Date.now()`/`new Date()` | `pnpm exec tsx --test testv2/projects-workspace-08-timeline.test.ts` |
| AC9 | Projection regression: service/secondary/readmodel tests stay green | `pnpm exec tsx --test testv2/projects-secondary-projection.test.ts testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts` |
| AC10 | Types + diff clean | `pnpm exec tsc --noEmit` · `git diff --check` |
| AC11 | App builds with the new Pane 2 branch | `pnpm exec next build --webpack` |
| HG | Timeline indentation/milestone marker + cross-view selection spot check | HUMAN GATE — not machine-verifiable |

## Assay plan (SCOPED — run exactly this)

```sh
pnpm exec tsx --test testv2/projects-workspace-08-timeline.test.ts
pnpm exec tsx --test testv2/projects-secondary-projection.test.ts testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts
pnpm exec tsc --noEmit
git diff --check
pnpm exec next build --webpack
```

Forbidden: `pnpm test`, `pnpm test:persistence`, `pnpm test:engine`, `pnpm test:app`, npm/yarn test,
any multi-file workflow glob. If `next build` cannot run, record the blocker and escalate — do not
fall back to the full harness.

## Risks (what could invalidate this plan)

1. **Inspector due-date write is browser-local noon (MED).** Pane 3 writes
   `new Date(`${date}T12:00:00`).toISOString()` (`components/portal/projects-workspace.tsx:757`),
   which can shift the portal calendar date for browsers far from UTC-04:00. The Timeline interprets
   the stored instant in portal TZ (C2). Accepted for WS-08 (read-only); the write fix belongs to
   WS-06. If the Lead touches the write path it becomes a SAME_UNIT addition needing its own test.
2. **WS-06 / WS-07 modules absent (MED).** `inspector-form.ts` and `work-actions.ts` do not exist in
   this base; do not import them. Selection sync targets the current Pane 3 `selectedNodeId` only.
3. **`ProjectWorkNode.dueAt` is optional and unvalidated (MED).** `attach` sets `dueAt` only when
   truthy (`ui/projects/service-projection.ts:156`); an invalid date string still reaches the node.
   `toPortalDateOnly` must return `null` for invalid input so bad data degrades to `undated`, never
   a crash or a bogus bucket.
4. **Fixture nodes are mostly undated (LOW).** `PROJECTS_WORKSPACE_FIXTURE` has few `dueAt` values
   (`ui/projects/source.ts`). The story test must construct explicit nodes with a fixed `today` and
   must not rely on the fixture.
5. **Timezone drift between test and runtime (MED).** Tests must pass a fixed `today` and fixed
   instants; never assert against `new Date()`.
6. **Route/geometry drift (LOW).** Timeline is a Pane 2 body change; do not alter Pane 1/2/3
   geometry or the view rail.
7. **Pre-existing `GuideItem` tsc noise (LOW).** Known; do not broaden scope to fix it.
8. **`next build` cost (LOW).** The component change warrants the build; if it cannot run, record the
   blocker (see Assay plan) rather than skip silently.

## Release obligations

- migrationRequired: false (reads existing `wbs_item.due_at` via the already-loaded projection).
- derivedRefreshRequired: false.
- deploymentRequired: true (Vercel UI on `/portal/projects`).
- DEV/PROD schema/data: untouched.

## Handoff to Lead

Build C1 (`timeline-projection.ts`, pure + fully unit-tested with a fixed `today`) and the
`toPortalDateOnly` helper FIRST so Assay is runnable before any JSX. Then wire C8 (Pane 2 branch +
`ProjectTimeline`) and C7 (selection dispatch/read). Extract ALL decision logic out of the React
component; the component only maps the projection to DOM and dispatches the existing intent. Report
exact files changed and the SCOPED Assay result. No commit/push from the architect node.

FORGE_PASS_STOP: COMPLETE
