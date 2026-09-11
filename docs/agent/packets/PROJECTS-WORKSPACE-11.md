# PROJECTS-WORKSPACE-11 — Project Activity projection (ARCHITECT CONTRACT)

Lane: architect. Design truth only. No implementation, no commit, no DEV/PROD write.

## Loop
intent: grow
parent_run: PROJECTS-WORKSPACE-09 (calendar projection) — batch 1
loop: 1/3

## Goal (one line)

Complete Activity as a bounded, project-scoped operational history: project its WBS/domain
events and its anchor-filtered communications into one stably-ordered list with an explicit
kind discriminator, actor/type/direction/summary/time, bounded pagination that never
duplicates a row, and distinct privacy-safe empty/unlinked/failure states that never fall
back to global activity.

## Non-goals (explicit)

- No schema change, no migration, no DDL. There is **no** `domain_event`/application event
  table and this story does not add one (DECIDED — DEFER; see Grounding).
- No new route and no new service domain. No new DB read: communications are the project's
  already-loaded, already-anchor-filtered `interaction` rows; WBS facts come from the
  project's already-loaded `wbs_item` rows; lifecycle comes from the already-loaded `project`
  row.
- No change to the global `/portal/activity` page or `getActivityFeed`'s SQL, join, or limit.
- No change to Work Plan, Timeline, Calendar, Documents, Financials, the view rail, or the
  three-pane geometry.
- No second selection store; Activity is read-only in this slice (no node selection required).
- No derivation of a WBS **completion** event from `wbs_item.updated_at` (there is no canonical
  completion timestamp; `updated_at` is a generic last-write proxy — Risk 4).
- No provider/OAuth read (`google_calendar_token_store` or any Apple/EventKit type). Activity
  reads canonical DB facts only.
- No listing special-casing; SCOPED only; no push/merge/deploy; no DEV/PROD write.

## Verified starting evidence (grounding)

- **There is no canonical domain-event store.** `docs/domain-event-persistence-decision.md`
  records the reviewed decision: **DEFER — do not build an application `domain_event` table**;
  "Nothing in the application writes a durable `domain_event`/`application_event` row today,
  and no command currently emits a `DomainEvent`." Therefore Activity's domain/WBS events MUST
  be *derived* from canonical facts, not read from an event log.
- **The canonical immutable timeline is `interaction`.** `interaction.event_type` defaults to
  `channel` (`db/migrations/005_crm_interaction_task_foundation.sql:8-37`); the checked channel
  set is `website|email|call|imessage|sms|calendar|meeting|showing|document|manual|note`.
  Communications and operational/relationship events share one table, distinguished by
  `channel`.
- **The existing read is narrow and bounded.** `getActivityFeed(limit)` inner-joins `person`,
  left-joins `property`/`deal`, orders `occurred_at desc`, and caps at `limit`
  (`db/activity-feed.ts:70-105`). The page calls `getActivityFeed(200)`
  (`app/portal/projects/page.tsx:120`). A property-only or personless interaction is invisible;
  older events fall outside the window. Absence is stated, never fabricated (Risk 2).
- **The anchor join already exists and is frozen.** `mapRealProjectsToWorkspace` derives
  `effectivePropertyIds`/`effectivePersonIds` (row anchor wins per type, else the distinct WBS
  entity anchors) and filters activity by stable id only, never by display name
  (`ui/projects/service-projection.ts:241-267`). WS-01 locked this
  (`testv2/projects-workspace-01-readmodel.test.ts:117-147`).
- **`plan.activity` shape is frozen by two tests.**
  `testv2/projects-service-projection.test.ts:146,163` and
  `testv2/projects-workspace-01-readmodel.test.ts:224` assert `plan.activity` is the raw
  filtered interaction list (`ids`, `length`). Activity MUST NOT mutate it; add a new field.
- **Activity is currently a shallow list.** `ProjectActivity`
  (`components/portal/projects-workspace.tsx:503-522`) reads `project.activity` and renders
  channel · direction / title / label with no kind discriminator, no WBS/domain events, no
  pagination, and falls to the generic `ProjectionState` (`:505`, `:588-613`, `:596` computes
  status only for documents/activity).
- **WBS facts carry canonical temporal fields.** `wbs_item.created_at` / `due_at` /
  `updated_at` exist and are selected (`db/migrations/124_wbs_project_item.sql:26-27`,
  `db/wbs-service-repository.ts:76-95`); `WbsItem.createdAt/dueAt/updatedAt` are typed
  (`services/wbs/types.ts:19-24`). There is **no** `completed_at`.
- **Project lifecycle facts exist end-to-end.** `project.created_at/starts_at/ends_at`
  (`db/migrations/125_project.sql`), selected by `SqlProjectRepository` and normalized to ISO
  (`db/project-service-repository.ts:59-62,78`); `Project.createdAt/startsAt/endsAt`
  (`services/project/types.ts:22-24`).
- **A pagination precedent exists** (`page`/`pageSize`/`total`, clamped): `db/catch-up.ts:66-135`,
  `db/client-admin.ts:136-226`. Activity reuses the shape, not the SQL.
- **Portal-time contract exists.** `PORTAL_TIME_ZONE_OFFSET='-04:00'` (PR, no DST),
  `toPortalInstant` (`lib/portal-time.ts:8,33`); the file has **no** date/time-label helper yet
  and no `server-only` import.
- **Page failure currently blanks everything.** `getActivityFeed(200)` is inside the
  `Promise.all` (`app/portal/projects/page.tsx:116-121`); a rejection makes the page `catch`
  return `domains:[], poles:[]` (`:138-142`) — Work Plan disappears. No fixture fallback exists
  (`:140`), so "no fixtures" is already true; "does not hide Work Plan" is the gap.
- **Precedent** for a pure React-free projection module + its own test:
  `ui/projects/secondary-projection.ts` + `testv2/projects-secondary-projection.test.ts`.
- **WS-08/WS-09 code is NOT in this base** (checkout `origin/main@edcf3d8`): no
  `ui/projects/timeline-projection.ts`, no `ui/projects/calendar-projection.ts`, no
  `lib/portal-time.ts#toPortalDateOnly`. Do not import them; keep WS-11 self-contained (Risk 3).

## Touched surfaces

Files
- `ui/projects/activity-projection.ts` (NEW, React-free, no `server-only`, no I/O, no
  `Date.now()`): `ProjectActivityKind`, `ProjectActivityItem`, `ProjectActivity`,
  `ProjectActivityPage`, `buildProjectActivity`, `paginateProjectActivity`, channel sets,
  and a local portal-stable label helper.
- `ui/projects/index.ts` — export the new module's types/functions.
- `ui/projects/model.ts` — **additive** `ProjectPlan.activityEvents?: ProjectActivityItem[]`,
  `ProjectPlan.activityState?: ProjectActivityState`, `ProjectPlan.activityNotice?: string | null`.
  `ProjectPlan.activity` is unchanged.
- `ui/projects/service-projection.ts` — add trailing optional `activityFailed = false`; keep the
  filtered-activity list in a local (`filteredActivity`) and pass it to `buildProjectActivity`;
  set `activityEvents`/`activityState`/`activityNotice`. `planActivity` and its output are
  unchanged.
- `components/portal/projects-workspace.tsx` — rewrite `ProjectActivity` to render the projected
  event list (kind badge, actor, type, direction, summary, portal time), bounded Prev/Next
  pagination, and distinct ready/empty/unlinked/failure copy (no generic `ProjectionState` for
  activity); hold page state via `useState`, reset on `project.id` change.
- `app/portal/projects/page.tsx` — isolate the `getActivityFeed` read so its failure is captured
  (`captureServerError("projects:load-activity", error, { level: "warn" })`) and degrades only the
  communications source; pass `activity = result ?? []` and `activityFailed = result === null`.
- `testv2/projects-workspace-11-activity.test.ts` (NEW — story-owned, red→green).

Tables
- Read-only: `interaction` via the existing `getActivityFeed` (communications/domain events);
  `wbs_item` via the already-loaded `ProjectPlan` inputs (WBS events); `project` row (lifecycle).
  No DDL, no new query. `google_calendar_token_store` / provider types are NOT read.

Routes
- `/portal/projects` only. No new route, no query param.

Tests
- NEW: `testv2/projects-workspace-11-activity.test.ts`.
- Existing, must stay green: `testv2/projects-service-projection.test.ts`,
  `testv2/projects-workspace-01-readmodel.test.ts`,
  `testv2/projects-secondary-projection.test.ts`.

## Contract

### C1 — Pure projection module (deterministic, no clock, no React)

`ui/projects/activity-projection.ts` is pure, total, and deterministic.

```ts
import type { WbsItem } from '@/services/wbs'
import type { Project } from '@/services/project'
import type { ActivityFeedEntry } from '@/db/activity-feed'

export type ProjectActivityKind = 'communication' | 'domain' | 'wbs' | 'lifecycle'

export type ProjectActivityItem = {
  id: string                 // stable, unique: 'interaction:<id>' | 'wbs:<itemId>:created'
                             //   | 'wbs:<itemId>:due' | 'project:created' | 'project:start' | 'project:end'
  kind: ProjectActivityKind
  type: string               // raw: channel, 'created', 'due', 'lifecycle'
  typeLabel: string          // human label
  actor: string | null       // personName ?? propertyName ?? WBS owner ?? null
  direction: string | null   // interaction direction; null for wbs/lifecycle
  summary: string            // primary line (never empty)
  detail: string | null      // secondary line when both title and summary exist
  occurredAt: string         // ISO instant (sort key)
  timeLabel: string          // portal-stable label (no browser TZ)
  nodeId: string | null      // WBS item id for wbs events; null otherwise
}

export type ProjectActivityState = 'ready' | 'empty' | 'unlinked' | 'failure'

export type ProjectActivity = {
  state: ProjectActivityState
  items: ProjectActivityItem[]   // full, sorted, deduped (unpaginated)
  total: number
  notice: string | null          // honest "source absent" copy; never fabricates an event
}

export type ProjectActivityPage = {
  state: ProjectActivityState
  items: ProjectActivityItem[]   // the requested page
  total: number
  page: number
  pageSize: number
  hasMore: boolean
  notice: string | null
}

export const COMMUNICATION_CHANNELS = ['website', 'email', 'call', 'imessage', 'sms'] as const
export const DOMAIN_CHANNELS = ['calendar', 'meeting', 'showing', 'document', 'manual', 'note'] as const
export const ACTIVITY_PAGE_SIZE = 20
export const ACTIVITY_MAX_PAGE_SIZE = 100

export function buildProjectActivity(input: {
  project: Pick<Project, 'id' | 'name' | 'status' | 'createdAt' | 'startsAt' | 'endsAt'> | null
  items: readonly WbsItem[]              // the selected project's WBS items
  activity: readonly ActivityFeedEntry[] // ALREADY anchor-filtered by the caller
  anchored: boolean
  failed?: boolean
}): ProjectActivity

export function paginateProjectActivity(
  items: readonly ProjectActivityItem[],
  page?: number,
  pageSize?: number,
): ProjectActivityPage
```

- Invariant: pure — same inputs → deep-equal output; no object-identity dependence.
- Invariant: total — never throws on malformed dates/ids; a bad instant degrades to no event.
- Invariant: no `Date.now()`/`new Date()` for "now"; the module never reads the machine clock.

### C2 — Event sources and the kind discriminator (AC: "WBS/domain events differ from communications")

The projection emits exactly four kinds from canonical facts:

- `communication` — `activity` entries whose `channel` is in `COMMUNICATION_CHANNELS`.
- `domain` — `activity` entries whose `channel` is in `DOMAIN_CHANNELS` (relationship/operational
  events already recorded on the canonical timeline).
- `wbs` — derived from `items`:
  - `created` at `item.createdAt` (valid instant only) → `id: wbs:<itemId>:created`;
  - `due` at `item.dueAt` (valid instant only) → `id: wbs:<itemId>:due`.
  - No `updated`/`completed` event (no canonical timestamp; see Non-goals/Risk 4).
- `lifecycle` — derived from `project`:
  - `created` at `project.createdAt` (valid) → `id: project:created`;
  - `start` at `project.startsAt` (valid) → `id: project:start`;
  - `end` at `project.endsAt` (valid) → `id: project:end`.

Field rules:
- `type` = raw channel / `'created'` / `'due'` / `'lifecycle'`; `typeLabel` = title-cased channel
  or a fixed phrase ("Work item created", "Work item due", "Project created", "Project starts",
  "Project ends").
- `actor` = interaction `personName ?? propertyName`, else WBS `owner`, else `null`.
- `direction` = interaction `direction`, else `null`.
- `summary` = interaction `title ?? summary ?? 'Activity recorded'`; WBS `Created "<title>"` /
  `Due "<title>"`; lifecycle fixed phrase. `detail` = interaction `summary` when `title` is also
  present, else `null`.
- `occurredAt` = the source instant (repository-normalized ISO). Invalid/missing → no event.

- Invariant: a `wbs`/`lifecycle`/`domain` event is never labelled or shaped as a
  `communication`; the `kind` field is the single discriminator.

### C3 — Stable total order and dedupe (AC: "pagination has no duplicates")

- Sort by `occurredAt` **descending**, tiebreak `id` **ascending** (lexicographic). Because
  `id` is unique this is a total order, so page slices can neither skip nor repeat a row.
- Dedupe by `id` (keep the first occurrence after sorting); the emitted list contains each `id`
  once.
- Invariant: flattening every page of a fixed `(items, pageSize)` yields the full ordered list
  exactly once; `total` equals `items.length` equals the number of distinct ids.

### C4 — Bounded pagination (AC: "bounded pagination")

- `paginateProjectActivity` clamps `page = max(1, floor(page ?? 1))` and
  `pageSize = min(ACTIVITY_MAX_PAGE_SIZE, max(1, floor(pageSize ?? ACTIVITY_PAGE_SIZE)))`.
- `items = ordered.slice((page-1)*pageSize, page*pageSize)`; `hasMore = page*pageSize < total`.
- Invariant: a page never exceeds `pageSize`; an out-of-range page returns `items: []` with the
  correct `total` and `hasMore:false`; the paginator is pure (no state, no mutation of input).

### C5 — Project scoping and missing anchors (AC: "every row relates to the project"; "missing anchors never return global activity")

- `activity` input MUST already be anchor-filtered by the caller (the same set that builds
  `planActivity`). `buildProjectActivity` NEVER re-reads or widens it and has no access to the
  global feed.
- `wbs` events come only from the project's `items`; `lifecycle` only from the selected
  `project`. There is no code path that emits an event for another project.
- If the project has no effective person/property anchor, the caller passes an empty `activity`
  array; the projection must therefore emit zero interaction events (never global activity).
- Invariant: for a project with no anchor, no `communication`/`domain` event can appear; the
  result may still contain that project's own `wbs`/`lifecycle` events.

### C6 — Distinct states and privacy-safe copy (AC: "privacy-safe empty/failure behavior")

- `state`:
  - `failure` — `failed === true` (the communications read failed).
  - `ready` — `items.length > 0`.
  - `unlinked` — `items.length === 0 && !anchored`.
  - `empty` — `items.length === 0 && anchored`.
- `notice` is derived, never invented:
  - `!anchored` → "This project is not linked to a contact or property, so linked
    communications cannot be shown."
  - `anchored` and zero interaction events → "No linked communications yet."
  - zero `wbs`/`lifecycle` events → "No project work items or lifecycle dates yet."
  - `null` when both families are present.
- Component copy (distinct, none is the generic "not available yet"):
  - `ready` → the event list (+ `notice` when present);
  - `empty` → "No project activity yet." (+ `notice`);
  - `unlinked` → "This project is not linked to a contact or property, so communications cannot
    be shown." (project `wbs`/`lifecycle` events still render);
  - `failure` → "Linked communications are temporarily unavailable. Project work history is
    unaffected." (available `wbs`/`lifecycle` events still render).
- Invariant: no fixture data is ever substituted; a failure never removes Work Plan; failure
  copy exposes no internal detail or other-project data.

### C7 — Failure isolation (communications failure does not hide Work Plan)

- `app/portal/projects/page.tsx` reads `getActivityFeed(200)` in a `.catch` that calls
  `captureServerError("projects:load-activity", error, { level: "warn" })` and resolves `null`.
  The `Promise.all` must no longer reject on an activity read failure, so WBS + project reads
  still compose and Work Plan renders. Pass `activity = result ?? []` and
  `activityFailed = result === null`.
- `mapRealProjectsToWorkspace(..., activityFailed = false)` threads the flag to
  `buildProjectActivity`, which returns `state:'failure'`.
- Invariant: an activity failure leaves `poles`, `projects`, and `workNodes` intact with
  `activityState:'failure'`; the failure is captured durably (never a silent fallback, never a
  fabricated event).

### C8 — Portal-timezone time semantics

- `timeLabel` for interaction events = the repository-normalized `entry.occurredAtLabel`
  (already produced by `getActivityFeed` in `America/Puerto_Rico`).
- `timeLabel` for `wbs`/`lifecycle` events is computed in the projection from the ISO instant
  using `PORTAL_TIME_ZONE_OFFSET` (`-04:00`, no DST) — shift the instant by the fixed offset and
  read UTC components; invalid/missing → no event.
- The helper is **local to `activity-projection.ts`** (imports only
  `PORTAL_TIME_ZONE_OFFSET`); WS-11 does not modify `lib/portal-time.ts`, avoiding the
  WS-08/WS-09 helper collision (Risk 3).
- Invariant: a rendered date/time does not depend on the browser timezone or the machine clock.
  The activity path contains no `toLocaleDateString`/`toLocaleTimeString`/`new Date().getDate()`
  rendering.

### C9 — Rendering

- Pane 2's `activeView === "activity"` branch renders `<ProjectActivity project={project} />`;
  all other branches unchanged.
- `ProjectActivity` reads `project.activityEvents ?? []` and holds `page` in `useState`
  (reset to 1 when `project.id` changes). It calls `paginateProjectActivity` with
  `ACTIVITY_PAGE_SIZE`.
- Each row renders: kind badge (`communication`/`domain`/`wbs`/`lifecycle`), `typeLabel`,
  `direction` when present, `actor` when present, `summary` (+ `detail`), and `timeLabel`.
  Rows are keyed by `item.id`.
- Pagination controls: Previous/Next (`disabled` at bounds), a "Showing A–B of N" line, and no
  page controls when `total <= pageSize`. Touch targets ≈48px (iPad).
- Invariant: geometry, view rail, and the other views are untouched; the activity path performs
  no browser-local date formatting.

## Acceptance (verifiable checks)

| # | Check | Assay command |
|---|-------|---------------|
| AC1 | Project scoping: only the project's WBS/lifecycle events and its anchor-filtered interactions appear; no other-project id can be emitted | `pnpm exec tsx --test testv2/projects-workspace-11-activity.test.ts` |
| AC2 | Kind discriminator: `communication` vs `domain` vs `wbs` vs `lifecycle` are distinct; WBS/domain events differ from communications in `kind`/`type`/`direction`/`actor` | `pnpm exec tsx --test testv2/projects-workspace-11-activity.test.ts` |
| AC3 | Pagination has no duplicates: flattening every page yields each id exactly once and equals the full ordered list; pageSize is clamped | `pnpm exec tsx --test testv2/projects-workspace-11-activity.test.ts` |
| AC4 | Missing anchors never return global activity: an unanchored project with a populated global feed yields zero interaction events; state is `unlinked` when no events remain | `pnpm exec tsx --test testv2/projects-workspace-11-activity.test.ts` |
| AC5 | Stable order: same inputs → deep-equal output; ties break by `id`; no `Date.now()`/`new Date()` in the module | `pnpm exec tsx --test testv2/projects-workspace-11-activity.test.ts` |
| AC6 | States/copy: `empty`/`unlinked`/`failure` messages are pairwise distinct and none is the generic "not available yet"; page catches the activity read via `captureServerError` (source-scan) | `pnpm exec tsx --test testv2/projects-workspace-11-activity.test.ts` |
| AC7 | Portal-time stable: a fixed instant's label is independent of the machine TZ; activity path has no `toLocaleDateString`/`toLocaleTimeString` | `pnpm exec tsx --test testv2/projects-workspace-11-activity.test.ts` |
| AC8 | Frozen-shape regression: `plan.activity` still maps by stable id and `activityEvents` is additive | `pnpm exec tsx --test testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts testv2/projects-secondary-projection.test.ts` |
| AC9 | Types + diff clean | `pnpm exec tsc --noEmit` · `git diff --check` |
| AC10 | App builds with the reworked Pane 2 branch + page isolation | `pnpm exec next build --webpack` |
| HG | Activity visual: kind badges, actor/direction, pagination, iPad, empty/unlinked/failure | HUMAN GATE — not machine-verifiable |

## Assay plan (SCOPED — run exactly this)

```sh
pnpm exec tsx --test testv2/projects-workspace-11-activity.test.ts
pnpm exec tsx --test testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts testv2/projects-secondary-projection.test.ts
pnpm exec tsc --noEmit
git diff --check
pnpm exec next build --webpack
```

Forbidden: `pnpm test`, `pnpm test:persistence`, `pnpm test:engine`, `pnpm test:app`, npm/yarn
test, any multi-file workflow glob. If `next build` cannot run, record the blocker and escalate —
do not fall back to the full harness.

## Risks (what could invalidate this plan)

1. **No canonical event store (HIGH).** `domain_event` is DEFER, so `domain`/`wbs`/`lifecycle`
   events are *derived* from `interaction` channels + `wbs_item.created_at/due_at` + `project`
   lifecycle dates. This is the deliberate design; do NOT invent an event table or a parallel
   store. If a reviewer requires a true event log, that is a separate story.
2. **`getActivityFeed` is narrower than the anchor join (MED).** It inner-joins `person` and
   limits to 200, so property-only/personless calendar/document interactions are invisible and
   older events fall outside the window. Absence is stated, not fabricated. Do NOT loosen the
   join or raise the limit in this story; note a follow-up.
3. **Shared-surface collision with WS-08/WS-09 (MED).** Those branches also touch
   `ui/projects/model.ts`, `ui/projects/service-projection.ts`,
   `components/portal/projects-workspace.tsx`, `app/portal/projects/page.tsx`, and
   `lib/portal-time.ts`. WS-11 keeps its label helper local to `activity-projection.ts` and its
   model/plan changes additive; the Lead must rebase/reconcile on whichever lands first. The
   `getActivityFeed` `.catch` isolation is identical to WS-09 C8 — land it once.
4. **No WBS completion timestamp (MED).** There is no `wbs_item.completed_at`; do NOT emit a
   "completed" event from `updated_at` (a generic last-write proxy would mislabel unrelated
   edits as completion). Follow-up: add `completed_at` if completion history is required.
5. **`plan.activity` shape is frozen (MED).** Two existing tests assert it. Keep it as the raw
   filtered interaction list; add `activityEvents`/`activityState`/`activityNotice` additively.
   Changing `plan.activity`'s ids to `interaction:<id>` would break
   `testv2/projects-service-projection.test.ts:146` and
   `testv2/projects-workspace-01-readmodel.test.ts:146`/`224`.
6. **Date-only test fixtures (LOW).** Existing projection tests pass `occurredAt:'2026-01-01'`.
   The activity test must use full ISO instants for order/label assertions and must not rely on
   `new Date()`.
7. **Derived `wbs`/`lifecycle` events are date-only facts (LOW).** They overlap Calendar
   (WS-09) by design — Activity is the unified chronological log, Calendar is the month grid.
   Keep the discriminator so the kinds never collapse.
8. **Pre-existing `GuideItem` tsc noise (LOW).** Known; do not broaden scope to fix it.
9. **`next build` cost (LOW).** The component + page change warrants the build; if it cannot
   run, record the blocker rather than skip silently.

## Release obligations

- migrationRequired: false (reads existing `interaction.channel`, `wbs_item.created_at/due_at`,
  `project.created_at/starts_at/ends_at`).
- derivedRefreshRequired: false.
- deploymentRequired: true (Vercel UI + page loader on `/portal/projects`).
- DEV/PROD schema/data: untouched.

## Handoff to Lead

Build C1 (`activity-projection.ts`, pure + fully unit-tested with fixed instants) FIRST so Assay
is runnable before any JSX. Then C2–C5 (sources, order, dedupe, pagination), then C6 (states +
copy), then C7 (page isolation + `activityFailed`), then C9 (component render + pagination).
Extract ALL decision logic out of the React component; the component only maps the projection to
DOM and pages it. Do not change `plan.activity`, `getActivityFeed`, `lib/portal-time.ts`, or any
other view. Report exact files changed and the SCOPED Assay result. No commit/push from the
architect node.
