# PROJECTS-WORKSPACE-06 — Selected-work inspector and safe edits (ARCHITECT CONTRACT)

Lane: architect. Design truth only. No implementation, no commit, no DEV/PROD write.

## Loop
intent: grow
parent_run: PROJECTS-WORKSPACE-05 (frozen read model + visual system)
loop: 1/3

## Goal (one line)

Make Pane 3 a reliable editor for a selected work item's status, due date, assignee, and notes — canonical app-user assignees, server-validated writes, idempotent Complete/Dismiss, stale-edit rejection, and refreshed reconciliation — without a schema change.

## Non-goals (explicit)

- No schema change, no migration, no `owner_app_user_id` column (see Risks for the follow-up).
- No new route, no new service domain; WBS keeps owning work items, Project keeps owning projects.
- No change to Pane 1/2 geometry, tree, projection ordering, or `project` writes.
- No change to `wbs_item` columns or to legacy `wbs_project`.
- No free-text assignee, no WhatsApp/person identity work.
- No full regression; SCOPED only.
- Do not special-case Casa Luar or any listing.

## Verified starting evidence (grounding)

- Pane 3 lives in `components/portal/projects-workspace.tsx:692` (`PaneThree`). Status/due/assignee/notes are local `useState`, reset only by `useEffect([node])` (`:699`).
- Assignee is a free-text `<input>` (`:750`). Owner is a `text` column on `wbs_item` (`db/migrations/124_wbs_project_item.sql:22`); legacy values are names/handles (`db/seeds/dev-projects-workspace.sql`), and `service-projection.ts:155` emits `owner` as the display string.
- Save calls `updateWbsItemAction` (`app/portal/wbs/actions.ts:84`) which does an unguarded read-modify-write (`wbs.get` then `wbs.save`) and passes EVERY field; `SqlWbsRepository.save` (`db/wbs-service-repository.ts:131`) sets every column unconditionally → last-write-wins clobber.
- Complete/Dismiss also route through the full save (`projects-workspace.tsx:761-762`) instead of the idempotent `complete`/`dismiss` repository ops (`db/wbs-service-repository.ts:152,164`).
- `WbsService.assertValid` (`services/wbs/wbs-service.ts:126`) validates only title/category.
- Canonical active-app-user read already exists: `listAssignableAgents()` (`db/person-admin.ts:151`) returns `{id, displayName}[]` for `app_user where active = true`.
- Repository normalizes timestamps to **millisecond ISO** (`db/wbs-service-repository.ts:36-40`); Postgres `updated_at` is microsecond. This is load-bearing for the precondition.
- `ProjectsWorkspaceData`/`ProjectWorkNode` carry no `updatedAt` and no assignee options today (`ui/projects/model.ts:29,117`).

## Touched surfaces

Files
- `components/portal/projects-workspace.tsx` — Pane 3 only (form controls, pending guard, feedback, unsaved-changes policy, dedicated complete/dismiss actions, submit `expectedUpdatedAt` + `ownerAppUserId`).
- `app/portal/projects/page.tsx` — load active app users; attach `assignees` to the mapped data.
- `app/portal/wbs/actions.ts` — `updateWbsItemAction`: add `expectedUpdatedAt` + `ownerAppUserId`, validate, resolve assignee, map `WBS_STALE_EDIT` / `WBS_INVALID_*`, capture failures via `captureServerError`.
- `services/wbs/types.ts` — extend `SaveWbsItemRequest` with `expectedUpdatedAt?: string | null`; add stale-edit domain error code.
- `services/wbs/wbs-service.ts` — save-shape validation; translate repository stale/not-found to domain errors.
- `services/wbs/repository.ts` — `save` contract (precondition + typed conflict/not-found).
- `db/wbs-service-repository.ts` — precondition in `save` WHERE; typed errors; no DDL.
- `ui/projects/model.ts` — add `ProjectWorkNode.updatedAt?: string`, `InspectorAssignee`, `ProjectsWorkspaceData.assignees?: InspectorAssignee[]`.
- `ui/projects/service-projection.ts` — map `updatedAt`; thread `assignees` through `mapRealProjectsToWorkspace`.
- `ui/projects/inspector-form.ts` (NEW, React-free) — pure validation/reconcile/selection-policy helpers.
- `ui/projects/index.ts` — export the new module.
- `db/auth-user.ts` — add `listActiveAppUsers()` (canonical app-user read).
- `db/person-admin.ts` — `listAssignableAgents` delegates to `listActiveAppUsers` (one source of truth).

Tables
- `wbs_item` — existing columns only (`status`, `due_at`, `owner`, `notes`, `updated_at`). No DDL.

Routes
- `/portal/projects` (server page + existing server actions). No new route.

Tests
- `testv2/projects-workspace-06-inspector.test.ts` (NEW), `testv2/wbs-service.test.ts`, `testv2/wbs-project-items-repository.test.ts`, `testv2/projects-workspace-01-readmodel.test.ts`.

## Contract

### C1 — Canonical app-user assignee

- New `listActiveAppUsers(): Promise<{id, displayName}[]>` in `db/auth-user.ts` (`app_user where active = true order by display_name asc`). `listAssignableAgents` delegates to it.
- `page.tsx` calls `listActiveAppUsers()` and attaches the result as `ProjectsWorkspaceData.assignees`; a read failure degrades to `[]` and is captured (`captureServerError`), never takes down the page.
- Pane 3 assignee control is a native `<select>`:
  - value = app_user **id**, plus a sentinel `""` = Unassigned.
  - options = `assignees`.
  - preselect = option whose `displayName` equals `node.owner`; if none matches (legacy handle), render a disabled `Current: <owner>` option so no data is silently dropped.
- On save the client submits `ownerAppUserId: string | null`.
- Server (`updateWbsItemAction`) resolves the id against `listActiveAppUsers()`:
  - null → `owner = null`.
  - not found/inactive → `{ok:false, code:"WBS_INVALID_ASSIGNEE"}`; **no write**.
  - found → persist the resolved canonical `displayName` into `wbs_item.owner`.
- Invariants
  - The only values written to `wbs_item.owner` are `null` or a canonical active app_user `display_name`.
  - Client-supplied free text is never persisted as `owner`.
  - WBS service stays unaware of `app_user` (resolution belongs to the action/edge).

### C2 — Save only intended fields + stale-edit precondition

- `ProjectWorkNode` gains `updatedAt?: string` (mapped from `WbsItem.updatedAt`); Pane 3 captures it at selection.
- Save submits `expectedUpdatedAt: string | null` (the captured node `updatedAt`).
- `SaveWbsItemRequest` gains `expectedUpdatedAt?: string | null`.
- `SqlWbsRepository.save`, when `expectedUpdatedAt` is provided:
  ```
  where id = ${id}
    and date_trunc('milliseconds', updated_at)
      = date_trunc('milliseconds', ${expectedUpdatedAt}::timestamptz)
  ```
  - 0 rows → re-`get(id)`: row exists ⇒ throw `WbsStaleEditError`; absent ⇒ throw `WbsNotFoundError`.
  - `WbsService` maps these to domain errors `WBS_STALE_EDIT` / `WBS_NOT_FOUND`.
- Save writes only `status`, `due_at`, `owner`, `notes` (+ `updated_at`). `title`, `category`, `project_id`, `parent_id`, `sort_order`, `entity_type`, `entity_id` are preserved from the fresh read and must not change.
- Invariants
  - A preconditioned save never overwrites a row whose `updated_at` advanced since the client's read; on conflict the row is unchanged.
  - Millisecond truncation is mandatory (repo normalizes to ms; DB is µs) — omitting it makes every save falsely stale.

### C3 — Validation

- Server-side, in the action (assignee) and `WbsService` (shape); client mirrors for UX only.
  - `status` ∈ {open, doing, done, dismissed} → else `WBS_INVALID_STATUS`.
  - `dueAt` null/"" → null; else must parse to a valid date → else `WBS_INVALID_DUE_AT`.
  - `notes` string ≤ 10_000 chars → else `WBS_NOTES_TOO_LONG`.
  - `ownerAppUserId` active app_user id or null → else `WBS_INVALID_ASSIGNEE`.
- Invariants: invalid input never reaches the repository; no partial write.

### C4 — Complete / Dismiss idempotent

- Pane 3 calls `completeWbsItemAction(node.id)` / `dismissWbsItemAction(node.id)` (existing dedicated ops, `db/wbs-service-repository.ts:152,164`), NOT the full save.
- Invariant: repeating complete/dismiss yields the same persisted status; the subsequent `router.refresh()` recomputes project progress and the next-action/action list from persisted state.

### C5 — Duplicate-submit protection

- Pane 3 holds one `pending` state gating ALL mutation controls, plus a `useRef` in-flight latch to block a second dispatch before re-render. Buttons get `disabled` + `aria-busy` while pending.
- Server: a duplicate save fails the C2 precondition (second is stale); complete/dismiss are idempotent by C4.

### C6 — Mutation feedback + refreshed reconciliation

- Feedback regions: success `role="status" aria-live="polite"` ("Saved"); failure `role="alert"`. Stale conflict renders "This item changed elsewhere. Reload to see the latest." with a Reload control.
- On success → `router.refresh()`; Pane 3 syncs its form to the fresh canonical node **only when not dirty**. When dirty and the incoming `updatedAt` differs from the last-seen value → show conflict; never overwrite in-progress edits.
- Pure `reconcileInspectorForm(current, incoming, {dirty, incomingUpdatedAt, lastSeenUpdatedAt})` owns this decision (React-free, unit-tested).

### C7 — Selection-change policy

- `shouldBlockSelectionChange({dirty})`. If dirty, a node selection change is blocked and an accessible inline bar renders "You have unsaved changes" with Discard / Keep editing (Esc = keep). Discard resets the form to the newly selected node and clears dirty.
- Invariant: unsaved edits are never silently discarded.

### C8 — Keyboard accessibility

- Every control is a native `select`/`input`/`textarea`/`button` with an explicit `id` and `<label htmlFor>`; visible focus ring; `type="button"`; pending state announced via `aria-busy`; status/error regions as C6; unsaved-changes bar reachable and dismissible via keyboard.

### Pure module API (`ui/projects/inspector-form.ts`)

```ts
export type InspectorAssignee = { id: string; displayName: string }
export type InspectorFormState = { status: ProjectWorkStatus; dueAt: string; owner: string; notes: string }
export type InspectorErrors = Partial<Record<'status'|'dueAt'|'owner'|'notes', string>>

export function inspectorFormFromNode(node: ProjectWorkNode): InspectorFormState
export function validateInspectorForm(state: InspectorFormState, assignees: InspectorAssignee[]): { ok: true } | { ok: false; errors: InspectorErrors }
export function reconcileInspectorForm(
  current: InspectorFormState,
  incoming: InspectorFormState,
  opts: { dirty: boolean; incomingUpdatedAt: string | null; lastSeenUpdatedAt: string | null },
): InspectorFormState
export function shouldBlockSelectionChange(opts: { dirty: boolean }): boolean
```

## Acceptance (verifiable checks)

| # | Check | Assay command |
|---|-------|---------------|
| AC1 | Save writes only intended fields; title/category/order/entity/project/parent preserved | `node --import tsx --test testv2/wbs-project-items-repository.test.ts` |
| AC2 | Complete/Dismiss idempotent; projection recomputes status/progress/next-action | `node --import tsx --test testv2/wbs-service.test.ts testv2/projects-workspace-01-readmodel.test.ts` |
| AC3 | Stale precondition → `WBS_STALE_EDIT`, zero rows changed, row intact | `node --import tsx --test testv2/wbs-project-items-repository.test.ts testv2/wbs-service.test.ts` |
| AC4 | Invalid assignee/due/status/notes rejected with no write | `node --import tsx --test testv2/projects-workspace-06-inspector.test.ts testv2/wbs-service.test.ts` |
| AC5 | Assignee resolved from active app_user; invalid/inactive rejected | `node --import tsx --test testv2/projects-workspace-06-inspector.test.ts` |
| AC6 | Reconcile + selection-change policy: dirty never overwritten/discarded | `node --import tsx --test testv2/projects-workspace-06-inspector.test.ts` |
| AC7 | Pane 3 uses labelled native controls + aria-live/role regions | `node --import tsx --test testv2/projects-workspace-06-inspector.test.ts` (source-scan) |
| AC8 | Types + diff clean; app builds | `node node_modules/typescript/bin/tsc --noEmit` · `git diff --check` · `pnpm exec next build --webpack` |

## Assay plan (SCOPED — run exactly this)

```sh
node --import tsx --test testv2/projects-workspace-06-inspector.test.ts testv2/wbs-service.test.ts testv2/wbs-project-items-repository.test.ts testv2/projects-workspace-01-readmodel.test.ts
node node_modules/typescript/bin/tsc --noEmit
git diff --check
pnpm exec next build --webpack
```

Forbidden: `pnpm test`, `pnpm test:persistence`, `pnpm test:engine`, `pnpm test:app`, npm/yarn test, multi-file workflow globs.

## Risks (what could invalidate this plan)

1. **Timestamp precision (HIGH).** Repo normalizes `updated_at` to ms; DB is µs. Without `date_trunc('milliseconds', …)` on both sides, every preconditioned save falsely fails stale. Assay must prove a matching-ms save succeeds and a mismatched one fails.
2. **Mixed owner semantics (MED).** Persisting canonical `display_name` is not rename-stable and is ambiguous under duplicate names; legacy `owner` handles don't preselect. Accepted for this slice. Follow-up story: add nullable `owner_app_user_id` and resolve display names at read.
3. **`router.refresh()` reconciliation race (MED).** A refresh while dirty could clobber edits; C6's dirty/last-seen guard is mandatory, not optional.
4. **Fixture degradation (LOW).** `InMemoryProjectsWorkspaceSource`/`PROJECTS_WORKSPACE_FIXTURE` have no `assignees`/`updatedAt`; Pane 3 must render Unassigned and skip the precondition when absent.
5. **Pre-existing `GuideItem` tsc noise (LOW).** Known issue; do not broaden scope to fix.
6. **Build scope (LOW).** `next build` is the integration check for UI + server-action changes; if it cannot run, record the blocker and escalate — do not fall back to the full harness.

## Release obligations

- migrationRequired: false (existing `wbs_item` columns only).
- derivedRefreshRequired: false.
- deploymentRequired: true (Vercel UI + server action).
- DEV/PROD schema/data: untouched.

## Handoff to Lead

Build the C1–C8 slices in order, starting with the pure module (`inspector-form.ts`) and repository/service precondition + validation so Assay is runnable before UI wiring. Extract all testable logic out of the React component. Report exact files changed and the SCOPED Assay result. No commit/push from this node.

FORGE_PASS_STOP: COMPLETE
