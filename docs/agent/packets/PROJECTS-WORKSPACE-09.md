# PROJECTS-WORKSPACE-09 — Project Calendar projection (ARCHITECT CONTRACT)

Lane: architect. Design truth only. No implementation, no commit, no DEV/PROD write.

## Loop
intent: grow
parent_run: PROJECTS-WORKSPACE-08 (timeline projection) — batch 1
loop: 1/3

## Goal (one line)

Complete Calendar as a selected-project view that renders the project's WBS commitments
(deadlines) plus its project lifecycle dates and only its canonically-anchored calendar
appointments, distinguishes the kinds, synchronizes WBS selection, renders in the portal
timezone, and fails in isolation without hiding Work Plan or substituting fixtures.

## Supersession note (read first)

The prior architect run produced a **list-based** calendar contract on base
`origin/main@ebf800c`. Commit `1073a31` (captain context, 2026-09-10) added the
**AMENDED scope**: reuse the shipped Catch-up calendar engine (month default, week
available) and derive key dates from `wbs_item.due_at` / `project.starts_at+ends_at` /
calendar intake. The prior contract's "No re-rendering the Calendar as a month grid …
this is a chronological, project-scoped list" is **superseded**. This contract replaces it.
The prior contract's failure-isolation, selection, purity, and no-fixture invariants are
**retained**.

## Non-goals (explicit)

- No schema change, no migration, no DDL. Every date source already exists.
- No new route and no new service domain. No new DB read: appointments are the project's
  already-loaded, already-anchor-filtered `interaction` rows with `channel = 'calendar'`.
- No new date store and no hand-entered key dates.
- **No second calendar engine.** Reuse the shipped Catch-up engine component + the shared
  normalized event boundary + the shared mappers.
- Do NOT read `google_calendar_token_store` (provider-side OAuth secret) or any
  Apple/EventKit/provider type. The pane reads canonical DB facts only.
- No change to the WBS due-date **write** path (browser-local noon in the Pane 3 inspector)
  — that is WS-06 scope (Risks 3).
- No second selection store: reuse `projects.selectNode` / `model.selectedNodeId`.
- No change to Work Plan, Timeline, Documents, Activity, Financials, the view rail, or the
  three-pane geometry.
- No widening the appointment channel set beyond `calendar` (meeting/showing = follow-up).
- No listing special-casing; SCOPED only; no push/merge/deploy; no DEV/PROD write.

## Verified starting evidence (grounding)

- **The live Catch-up engine is `FullCalendarCandidate`, not the ilamy candidate.**
  `app/portal/catch-up/page.tsx:1,39` renders `CatchUpBoard`
  (`components/portal/wbs/catch-up-board.tsx:8,231`) which renders `FullCalendarCandidate`
  with `initialView="dayGridMonth"` and toolbar `dayGridMonth,timeGridWeek,timeGridDay`
  (`components/portal/fullcalendar-candidate.tsx:51-64`) — month default, week available.
  `components/portal/catch-up-calendar.tsx` (ilamy, `initialView="month"`) is rendered ONLY
  by the A/B evaluation harness, which `workflow_app/tests/catch-up.test.ts:254` asserts is
  **not rendered**. So the shipped engine = `FullCalendarCandidate`.
- **Shared normalized boundary + mappers already exist.** `CatchUpCalendarEvent` +
  `normalizeCalendarEvent` (`lib/catchup/calendar-adapter.ts:13-71`); `toIlamyCalendarEvent`
  / `toFullCalendarEvent` (`lib/catchup/calendar-mappers.ts:21-50`) color `kind === 'showing'`
  gold (`#c6a15b`) else blue (`#3f6ea5`). No provider types.
- **Calendar is currently a shallow list.** `ProjectCalendar`
  (`components/portal/projects-workspace.tsx:467-483`) reads `project.calendarItems` and
  renders a flat list; empty falls to `ProjectionState view="calendar"` (`:469`) whose status
  is computed only for documents/activity (`:596`), yielding the generic
  "Calendar is not available in this workspace yet." copy (`:604`).
- **Dates render browser-locally.** `new Date(item.startAt).toLocaleDateString(undefined, …)`
  (`:475`) — the "timezone stable" AC is currently violated.
- **Deadline projection is pure and tested.** `mapProjectCalendarItems`
  (`ui/projects/secondary-projection.ts:13-18`) keeps valid `dueAt`, stable order;
  `testv2/projects-secondary-projection.test.ts` asserts it. Keep its shape.
- **WBS due dates are the commitments.** `WbsItem.dueAt` (`services/wbs/types.ts:19`);
  `wbs_item.due_at timestamptz` (`db/migrations/124_wbs_project_item.sql`).
- **Project lifecycle dates exist end-to-end.** `Project.startsAt/endsAt`
  (`services/project/types.ts:22-23`); selected by `SqlProjectRepository`
  (`db/project-service-repository.ts:59-60,78`); `project.starts_at/ends_at timestamptz`
  (`db/migrations/125_project.sql:11-12`).
- **Appointments are canonical interactions on channel `calendar`.** `adaptCalendarEvent`
  emits `channel:'calendar'`, `eventType:'appointment'`
  (`lib/crm-calendar-normalization.ts:250-260`); `getActivityFeed` reads `interaction` and
  preserves `channel` (`db/activity-feed.ts:70-105`). `calendar_intake_receipt.interaction_id`
  (`db/migrations/040_calendar_intake_receipt.sql:53`) is the provenance link;
  `google_calendar_token_store` (`db/migrations/041…`) is provider-side and is NOT read.
- **The anchor join already exists.** `effectivePropertyIds`/`effectivePersonIds` id
  intersection (`ui/projects/service-projection.ts:241-267`); `planActivity` is already
  project-scoped by stable id (never by display name).
- **Operating-timezone contract exists.** `PORTAL_TIME_ZONE_OFFSET='-04:00'` (PR, no DST),
  `toPortalInstant` (`lib/portal-time.ts:8,33`); no `server-only` import, so it is safe from
  the `"use client"` component. `lib/portal-time.ts` has no date-only/time-label helper yet.
- **Selection is centralized.** `model.selectedNodeId`; intents `projects.selectNode` /
  `projects.selectProject` (`ui/projects/model.ts:159-167`); `PaneTwo` already threads
  `selectedNodeId`/`onSelectNode` (`components/portal/projects-workspace.tsx:627-668`);
  `selectProject/Pole/Domain` reset `activeView:"work-plan"` — the Calendar must dispatch
  ONLY `selectNode`.
- **Page failure currently blanks everything.** `getActivityFeed(200)` is inside the
  `Promise.all` (`app/portal/projects/page.tsx:116-121`); a rejection makes the page `catch`
  return `domains:[], poles:[]` (`:138-142`) — Work Plan disappears. No fixture fallback
  already exists (`:140`), so "no fixtures" is already true; "does not hide Work Plan" is
  the gap.
- **Precedent** for a pure React-free projection module + its own test:
  `ui/projects/secondary-projection.ts` + `testv2/projects-secondary-projection.test.ts`.
- **WS-08 code is NOT in this base**: `ui/projects/timeline-projection.ts` and
  `lib/portal-time.ts#toPortalDateOnly` do not exist. Do not import them (Risks 2).

## Touched surfaces

Files
- `lib/catchup/calendar-adapter.ts` — **additive** optional fields on `CatchUpCalendarEvent`:
  `variant?: 'deadline' | 'appointment' | 'lifecycle'` and `nodeId?: string | null`.
  `normalizeCalendarEvent` and all existing producers are unchanged (fields optional).
- `lib/catchup/calendar-mappers.ts` — color by `variant` when present, else the existing
  `kind === 'showing'` fallback (Catch-up output unchanged). `toFullCalendarEvent` carries
  `extendedProps: { variant, nodeId }` so `eventClick` can select a WBS node.
- `components/portal/fullcalendar-candidate.tsx` — **additive optional props**
  `onEventSelect?: (nodeId: string) => void` and `selectedNodeId?: string | null` wired to
  `eventClick` + event classNames. Default (no props) behavior is byte-for-byte Catch-up.
- `ui/projects/calendar-projection.ts` (NEW, React-free, no `server-only`, no I/O, no
  `Date.now()`): `ProjectCalendarEvent`, `ProjectCalendarState`, `ProjectCalendar`,
  `buildProjectCalendar`.
- `ui/projects/model.ts` — `ProjectPlan.calendarItems` becomes `ProjectCalendarEvent[]`; add
  `calendarState?: ProjectCalendarState` and `calendarNotice?: string | null`.
- `ui/projects/service-projection.ts` — compose the calendar from the project's WBS
  deadlines + lifecycle dates + the already-anchor-filtered calendar appointments; add an
  optional trailing `calendarFailed = false`; update calendar provenance.
- `ui/projects/index.ts` — export the new module's types/functions.
- `components/portal/projects-workspace.tsx` — rewrite `ProjectCalendar` to render the
  shipped engine via the shared mappers, distinguish deadline/appointment/lifecycle, sync
  selection, render portal-stable copy for ready/empty/failure (no generic `ProjectionState`
  for calendar); thread `selectedNodeId`/`onSelectNode`.
- `app/portal/projects/page.tsx` — isolate the `getActivityFeed` read so its failure is
  captured and degrades only the appointment feed (never the whole page); pass
  `calendarFailed`.
- `testv2/projects-workspace-09-calendar.test.ts` (NEW — story-owned, red→green).

Tables
- Read-only: `wbs_item` (deadlines), `project.starts_at/ends_at` (lifecycle),
  `interaction` via the existing `getActivityFeed` (appointments). No DDL, no new query.
  `calendar_intake_receipt` / `google_calendar_token_store` are NOT read by the pane.

Routes
- `/portal/projects` only. No new route.

Tests
- NEW: `testv2/projects-workspace-09-calendar.test.ts`.
- Existing, must stay green: `testv2/projects-secondary-projection.test.ts`,
  `testv2/projects-service-projection.test.ts`, `testv2/projects-workspace-01-readmodel.test.ts`.

## Contract

### C1 — Engine-agnostic projection (the key architectural decision)

The projection MUST emit the shared normalized boundary `CatchUpCalendarEvent[]` (extended
with `variant`/`nodeId`), so the pane renders through the **existing** Catch-up engine and
its **existing** mappers. The engine is a renderer, not a data model. This is why the exact
engine component is a one-line swap with no contract change: `FullCalendarCandidate` is
selected because it is the component the live `/portal/catch-up` route actually renders.

`ui/projects/calendar-projection.ts` — pure, total, deterministic, no clock, no React:

```ts
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'
import type { Project, WbsItem } from '@/services/…'
import type { ActivityFeedEntry } from '@/db/activity-feed'

export type ProjectCalendarEventKind = 'deadline' | 'appointment' | 'lifecycle'

export type ProjectCalendarEvent = CatchUpCalendarEvent & {
  variant: ProjectCalendarEventKind
  nodeId: string | null          // deadline => WBS id (selectable); others => null
  status: string | null          // deadline => WBS status; others => null
}

export type ProjectCalendarState = 'ready' | 'empty' | 'failure'

export type ProjectCalendar = {
  state: Exclude<ProjectCalendarState, 'failure'> // pure fn never invents I/O failure
  events: ProjectCalendarEvent[]
  notice: string | null          // honest "source absent" copy; never fabricates an event
}

export function buildProjectCalendar(input: {
  project: Pick<Project, 'name' | 'startsAt' | 'endsAt'>
  deadlines: readonly WbsItem[]        // the selected project's WBS items
  appointments: readonly ActivityFeedEntry[] // ALREADY anchor-filtered by the caller
  anchored: boolean                    // project has a person or property anchor
}): ProjectCalendar
```

- Invariant: pure — same inputs → deep-equal output; no dependence on object identity.
- Invariant: total — never throws on malformed dates/ids; bad input degrades to no event.

### C2 — Deadline events (retain WBS commitment projection)

- Reuse `mapProjectCalendarItems(deadlines)` internally (valid `dueAt` only, stable order).
  Do NOT change that function's shape: `testv2/projects-secondary-projection.test.ts`
  asserts it.
- Each item → `variant:'deadline'`, `kind:'other'`, `allDay:true`, `nodeId:item.id`,
  `status:item.status`, `title:item.title`, `startAt:item.startAt`, `endAt:null`,
  `personId:null`, `personName:item.owner`, `propertyName:null`, `source:'wbs'`.
- Invariant (AC "no due date creates no event"): `null`, missing, or unparseable `dueAt`
  yields no event.

### C3 — Lifecycle events (project.starts_at / project.ends_at)

- If `project.startsAt` is a valid instant → one event `id:'project-start'`,
  `title:'<project name> starts'`, `variant:'lifecycle'`, `allDay:true`, `nodeId:null`,
  `status:null`, `source:'project'`.
- If `project.endsAt` is valid → `id:'project-end'`, `title:'<project name> ends'`.
- Invariant: a null/unparseable lifecycle date creates no event (never a placeholder).

### C4 — Appointment events (canonically linked only)

- Input `appointments` MUST already be anchor-filtered to the project by the caller
  (`service-projection.ts` reuses the same filtered set that builds `planActivity`).
- The projection additionally filters to `entry.channel === 'calendar'`.
- Map → `variant:'appointment'`, `kind:'other'`, `allDay:false`, `nodeId:null`,
  `status:null`, `title: entry.title ?? entry.summary ?? 'Appointment'`,
  `startAt: entry.occurredAt`, `endAt:null`, `personId: entry.personId`,
  `personName: entry.personName`, `propertyName: entry.propertyName`, `source:'calendar'`.
- Invariant (AC "only selected-project events appear"): an appointment whose person/property
  id does not intersect the selected project's effective anchors is never an input, and an
  invalid `occurredAt` yields no event.
- The pane reads canonical `interaction` facts only. It never reads
  `google_calendar_token_store` or any provider/Apple type.

### C5 — Distinction + stable order

- `variant` is the discriminator; `allDay` and `nodeId` reinforce it (deadlines/lifecycle
  are date-only; deadlines are WBS-selectable; appointments carry a time and are not).
- Mapper colors: deadline `#c6a15b` (gold), appointment `#3f6ea5` (blue), lifecycle
  `#030f23` (navy). Existing Catch-up events (no `variant`) keep today's colors.
- Order: `startAt` ascending (ISO lexicographic), tiebreak `id` ascending. Deterministic.
- Invariant: flattening yields each event once; deadline count = valid-dated deadline count;
  appointment count = channel-calendar appointment count.

### C6 — Portal-timezone date/time semantics

- Add pure `toPortalDateOnly(instant): string | null` and
  `toPortalTimeLabel(instant): string | null` to `lib/portal-time.ts`, reusing
  `PORTAL_TIME_ZONE_OFFSET`. Shift the instant by the fixed `-04:00` offset and read UTC
  components; invalid/missing → `null`.
- **If a sibling worktree already added `toPortalDateOnly`, reuse it; do not redefine it.**
- Invariant: an instant's rendered date/time does not depend on the browser's timezone or the
  machine clock. The calendar path contains no `toLocaleDateString`/`toLocaleTimeString`/
  `new Date(...).getDate()` rendering. The engine itself formats in local time; therefore the
  projection normalizes each instant to the portal wall-clock value before it reaches the
  engine (or the engine is fed portal-shifted instants) so the displayed day/time is stable.

### C7 — Distinct states, honest absence, no fixtures

- `buildProjectCalendar` returns `state:'ready'` when `events.length > 0`, else `'empty'`.
- `notice` is derived, never invented:
  - not `anchored` → "This project is not linked to a contact or property, so linked
    appointments cannot be shown." (appointments source absent — stated, not dropped);
  - anchored but zero calendar appointments → "No linked appointments yet.";
  - zero WBS deadlines → "No WBS deadlines yet."
  - `null` when both kinds are present.
- `mapRealProjectsToWorkspace` sets `calendarState:'failure'` when the caller reports
  `calendarFailed:true`; otherwise the pure `state`.
- Component copy (distinct, none is the generic "not available yet"):
  - `ready` → the engine, plus `notice` if present;
  - `empty` → "No deadlines or appointments for this project yet." plus `notice`;
  - `failure` → "Linked appointments are temporarily unavailable. Work Plan is unaffected."
    and any available deadline/lifecycle events still render.
- Invariant: no fixture data is ever substituted; a failure never removes Work Plan.

### C8 — Failure isolation (Calendar failure does not hide Work Plan)

- `app/portal/projects/page.tsx` reads `getActivityFeed(200)` in a `.catch` that calls
  `captureServerError("projects:load-activity", error, { level: "warn" })` and resolves
  `null`; the page passes `activity = result ?? []` and `calendarFailed = result === null`.
  The `Promise.all` must no longer reject on an activity/calendar read failure, so the WBS +
  project reads still compose and Work Plan renders.
- `buildProjectCalendar` is total, so a projection error cannot throw out of
  `mapRealProjectsToWorkspace`.
- Invariant: a calendar/appointment failure leaves `poles`, `projects`, and `workNodes`
  intact with `calendarState:'failure'`; the failure is captured durably (never a silent
  fallback, never a fabricated event).

### C9 — WBS selection synchronization

- `ProjectCalendar` receives `selectedNodeId` and `onSelectNode` (threaded from `PaneTwo`,
  already available).
- Pass `onEventSelect={onSelectNode}` and `selectedNodeId` to `FullCalendarCandidate`; a
  deadline event with a `nodeId` selects that WBS node on click and is visually marked when
  `nodeId === selectedNodeId`. Appointment/lifecycle events are non-interactive.
- Selecting from the Calendar dispatches ONLY `selectNode` (never `selectProject`/
  `selectPole`/`selectDomain`, which reset `activeView` to `work-plan`).
- Invariant: no second selection store; the same `selectedNodeId` drives Work Plan highlight
  and the Pane 3 inspector.

### C10 — Rendering

- Pane 2's `activeView === "calendar"` branch passes `project`, `selectedNodeId`,
  `onSelectNode` to `ProjectCalendar`; all other branches unchanged.
- `ProjectCalendar` renders the shipped engine:
  `<FullCalendarCandidate events={project.calendarItems ?? []} heading="Project calendar"
   onEventSelect={onSelectNode} selectedNodeId={selectedNodeId} />`
  with month default and week available, exactly as Catch-up does. No new engine, no grid.
- Invariant: geometry, view rail, and the other views are untouched.

## Acceptance (verifiable checks)

| # | Check | Assay command |
|---|-------|---------------|
| AC1 | Only selected-project events appear: non-anchored appointments excluded; deadlines come only from the project's WBS | `pnpm exec tsx --test testv2/projects-workspace-09-calendar.test.ts` |
| AC2 | No due date creates no event: undated/invalid WBS items, lifecycle dates, and instants produce no event | `pnpm exec tsx --test testv2/projects-workspace-09-calendar.test.ts` |
| AC3 | Timezone stable: date-only/time derive from the portal offset independent of browser TZ; calendar path has no `toLocaleDateString`/`toLocaleTimeString` | `pnpm exec tsx --test testv2/projects-workspace-09-calendar.test.ts` |
| AC4 | Deadline vs appointment vs lifecycle distinguished (`variant`/`allDay`/`nodeId`) and ordered deterministically; shared mapper color fallback unchanged for Catch-up events | `pnpm exec tsx --test testv2/projects-workspace-09-calendar.test.ts` |
| AC5 | Failure isolation: `calendarState:'failure'` while `poles`/`workNodes` intact; page catches the activity read; no fixture substitution | `pnpm exec tsx --test testv2/projects-workspace-09-calendar.test.ts` |
| AC6 | Selection sync: source-scan proves Calendar dispatches `onSelectNode`/`selectNode`, never `selectProject`; highlight derives from `selectedNodeId` | `pnpm exec tsx --test testv2/projects-workspace-09-calendar.test.ts` |
| AC7 | Engine reuse: source-scan proves `ProjectCalendar` renders `FullCalendarCandidate` + `CatchUpCalendarEvent`/`calendar-mappers` and creates no second engine | `pnpm exec tsx --test testv2/projects-workspace-09-calendar.test.ts` |
| AC8 | Empty vs failure copy are distinct and neither is the generic "not available yet" | `pnpm exec tsx --test testv2/projects-workspace-09-calendar.test.ts` |
| AC9 | Projection regression: secondary/service/readmodel tests stay green | `pnpm exec tsx --test testv2/projects-secondary-projection.test.ts testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts` |
| AC10 | Types + diff clean | `pnpm exec tsc --noEmit` · `git diff --check` |
| AC11 | App builds with the reworked Pane 2 branch + page isolation | `pnpm exec next build --webpack` |
| HG | Calendar visual: deadline vs appointment, portal date/time, selection, iPad | HUMAN GATE — not machine-verifiable |

## Assay plan (SCOPED — run exactly this)

```sh
pnpm exec tsx --test testv2/projects-workspace-09-calendar.test.ts
pnpm exec tsx --test testv2/projects-secondary-projection.test.ts testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts
pnpm exec tsc --noEmit
git diff --check
pnpm exec next build --webpack
```

Adjacent check (only if `lib/catchup/calendar-*` or `fullcalendar-candidate.tsx` were
changed; SINGLE FILE, never a glob, and only if the runtime policy permits it):
`pnpm exec tsx --test workflow_app/tests/catch-up.test.ts`. If that command is judged to
violate SCOPED policy, record it as a blocker instead — do NOT fall back to `pnpm test:app`.

Forbidden: `pnpm test`, `pnpm test:persistence`, `pnpm test:engine`, `pnpm test:app`, npm/yarn
test, any multi-file workflow glob. If `next build` cannot run, record the blocker and
escalate — do not fall back to the full harness.

## Risks (what could invalidate this plan)

1. **Engine-choice drift (MED).** The captain names `catch-up-calendar.tsx` (ilamy) but the
   live route renders `FullCalendarCandidate` (evidence above). This contract reuses
   `FullCalendarCandidate` and keeps the projection engine-agnostic so the swap is one line.
   Lead: do NOT introduce both — that is the "second engine" the amendment forbids.
2. **Shared Catch-up code touched (MED).** Adding `variant`/`nodeId`/`extendedProps` to the
   adapter/mappers and optional props to `FullCalendarCandidate` must be strictly additive;
   no `variant` ⇒ identical Catch-up output. Unit-test the fallback in the new testv2 file.
3. **`toPortalDateOnly` collision with WS-08 (MED).** WS-08 plans the same helper. If both
   land, keep ONE definition. Lead: check for an existing export and reuse it.
4. **Engine formats in local time (MED).** `FullCalendar`/ilamy render using the browser
   timezone. To satisfy AC3, normalize each instant to the portal wall-clock value in the
   projection (or feed portal-shifted instants) so the rendered day/time is portal-stable;
   verify with a fixed-instant test, not the machine clock.
5. **Appointment read is narrower than the anchor join (MED).** `getActivityFeed` inner-joins
   `person` and limits to 200 rows, so a property-only or personless calendar interaction is
   invisible and older appointments fall outside the window. Absence is stated, not
   fabricated. Do NOT loosen the join or raise the limit in this story; note a follow-up.
6. **Inspector due-date write is browser-local noon (MED).** Pane 3 writes
   `new Date(`${date}T12:00:00`).toISOString()`; far-from-UTC-04:00 browsers can shift the
   portal date. Calendar reads in portal TZ. Accepted for WS-09; the write fix is WS-06.
   Touching the write path makes it a SAME_UNIT addition needing its own test.
7. **Retained `mapProjectCalendarItems` shape (MED).** Changing its return type breaks
   `testv2/projects-secondary-projection.test.ts`. Keep it and layer `buildProjectCalendar`
   on top.
8. **`calendarItems` type change (LOW).** `testv2/projects-workspace-01-readmodel.test.ts:222`
   asserts only `calendarItems?.length === 1`; a richer event array satisfies it.
9. **Fixture `appointment`-type nodes are not a real source (LOW).** `ProjectWorkNodeType`
   includes `appointment` and the fixture uses it, but no WBS category maps to it; do not
   treat fixture appointments as canonical and do not invent a WBS appointment source.
10. **Page isolation changes shared read (MED).** Catching `getActivityFeed` also changes the
    Activity view's failure behavior (empty vs page failure). Keep it minimal and captured;
    do not alter Activity's data contract.
11. **Pre-existing `GuideItem` tsc noise (LOW).** Known; do not broaden scope to fix it.
12. **`next build` cost (LOW).** The component + page change warrants the build; if it cannot
    run, record the blocker rather than skip silently.

## Release obligations

- migrationRequired: false (reads existing `wbs_item.due_at`, `project.starts_at/ends_at`,
    `interaction.channel`).
- derivedRefreshRequired: false.
- deploymentRequired: true (Vercel UI + page loader on `/portal/projects`).
- DEV/PROD schema/data: untouched.

## Handoff to Lead

Build C1 (`calendar-projection.ts`, pure + fully unit-tested) and C6
(`toPortalDateOnly`/`toPortalTimeLabel`) FIRST so Assay is runnable before any JSX. Then make
the shared-engine extension additive (C5 adapter/mapper variant + `extendedProps`), then wire
C2/C3/C4 through `service-projection.ts` (appointments = the same anchor-filtered set that
builds `planActivity`, filtered to `channel === 'calendar'`), then C8 (page isolation +
`calendarFailed`), then C9/C10 (component selection + engine render). Extract ALL decision
logic out of the React component; the component only maps the projection to the engine and
dispatches the existing `selectNode` intent. Report exact files changed and the SCOPED Assay
result. No commit/push from the architect node.
