# PROJECTS-WORKSPACE-13 — Responsive, accessibility, and visual acceptance (ARCHITECT CONTRACT)

Lane: architect. Design truth only. No implementation, no commit, no DEV/PROD write.

## Loop
intent: grow
parent_run: PROJECTS-WORKSPACE-12 (URL/breadcrumb/shareable context) — batch 1
loop: 1/3

## Goal (one line)

Make the complete `/portal/projects` workspace operable and legible on desktop, iPad,
keyboard, and screen readers by defining a responsive pane model (inline → drawers),
correct ARIA semantics for the navigator tree / domain+view tabs / work-plan grid /
inspector forms, focus transfer for every overlay, announced async feedback, and ≥44px
touch targets — while preserving the frozen desktop hierarchy (navigator → canvas →
inspector) and surface system exactly.

## Non-goals (explicit)

- No schema change, no migration, no DDL, no new DB read, no new route. `/portal/projects`
  stays `force-dynamic` and reads the same authorized payload.
- **No change to the frozen desktop geometry values** (navigator `minmax(350px,375px)`,
  canvas `minmax(0,1fr)`, inspector `minmax(295px,315px)`), pane order, surface families,
  colors, or `PROJECTS_LONG_CONTENT` policies. `testv2/projects-workspace-02-tokens.test.ts`
  must stay green unmodified.
- No browser, screenshot, Playwright, axe, or jest-dom tooling. This repo is `node:test` +
  `tsx` only; pixel comparison, real keyboard traversal, screen-reader announcement, and
  iPad touch are the **WS-18 HUMAN GATE**, not a machine assay here (see HG).
- No new navigation/selection store. Drawer open/closed is transient local UI state, never a
  second authority over `ProjectsWorkspaceController` selection.
- No change to `plan.*` shapes, `service-projection.ts`, controller selection semantics,
  `getActivityFeed`, `lib/portal-time.ts`, the global `CommandPalette` catalog/mount, or any
  other portal surface.
- No replacement/fork of `react-arborist`; semantics are added around/through its public
  props only.
- No visual redesign, no new colors, no geometry change below the breakpoints beyond the
  placement model defined here.
- No listing special-casing; SCOPED only; no push/merge/deploy; no DEV/PROD write.

## Verified starting evidence (grounding)

- **The desktop grid is the only designed layout.** `.projects-workspace-grid` is
  `display:grid` at all widths but `grid-template-columns` is set **only** inside
  `@media (min-width: 1024px)` (`app/globals.css:602-611`). Below 1024px it falls to a
  single auto column; panes are `overflow-hidden` with no bounded height
  (`ui/projects/visual-system.ts:51-52`), so the navigator tree (measured height, min 200 —
  `projects-workspace.tsx:228-241,395`) and the canvas/inspector clip or stack unintentionally.
  This is the concrete "panes clip on iPad/small" defect WS-13 fixes.
- **The viewport-bound height is desktop-only and shell-tokenized.**
  `height: calc(100dvh - var(--portal-shell-chrome-height, 10.5rem))` at `lg`
  (`app/globals.css:606-611`); the shell owns `--portal-shell-chrome-height: 10.5rem`
  (`app/globals.css:594-596`). No medium/small strategy exists.
- **The view rail is not a tabs widget.** It is a `<nav aria-label="Project workspace views">`
  of buttons with `aria-current` (`projects-workspace.tsx:639-656`); no `role="tablist"`,
  `aria-selected`, `aria-controls`, or `tabpanel`. The content region has no role
  (`:657-669`).
- **The domain rail is not a tabs/radiogroup.** A plain `<div aria-label="Project domain">`
  of buttons with `aria-current` (`:189-226`); it swaps the navigator tree but exposes no
  selected/tab semantics.
- **The navigator tree is `react-arborist`.** `<Tree>` with `searchMatch`, roving selection,
  and per-row `ToggleButton` whose label is only `"Collapse"`/`"Expand"` with no node context
  (`:270-285`). The tree container is unlabelled.
- **Work Plan is a visual table without table semantics.** A hand-rolled header grid
  (`grid-cols-[22px_minmax(0,1fr)_72px_88px]`) over nested `<ul>`/`<li>`/`<button>`
  (`:454-465,424-452`) — no `role="treegrid"`, `columnheader`, `rowheader`, `gridcell`,
  `aria-level`, or `aria-expanded`; hierarchy is conveyed only visually by indent.
- **The inspector form is partially labelled but not announced.** Controls are wrapped in
  `<label>` (`:738-755`) so they have names, but there is no `aria-describedby`, no
  `aria-invalid`, no `role="alert"`/`aria-live`, and `saveError` renders as plain text
  (`:756`). Save status ("Saving…", `:758`) is not announced. Touch heights are inconsistent
  (`text-[9px]` status `<select>` at `:541`, `px-2 py-1` inputs at `:740-750`).
- **Overlays have no focus contract.** The New Project popover is an absolutely positioned
  `<div>` (`:966-991`) with `autoFocus` on the input and a `×` close button but no
  `role="dialog"`, `aria-modal`, accessible name, Escape handler, focus trap, or focus
  restore. The established overlay precedent is `CommandPalette` (`role="dialog"`,
  `aria-modal`, Escape, backdrop button, autofocus — `command-palette.tsx:130-162`) but it too
  lacks a trap/restore; WS-13 defines the contract and may reuse the same shape.
- **Reusable form + live patterns already exist.** `portalControlClass` is `min-h-11`
  (44px) with `aria-[invalid=true]` styling (`components/portal/ui/portal-field.tsx:5-6`);
  `PortalFieldError` is `role="alert"` (`:49-67`); `CommandStatus` uses `aria-live="polite"`
  (`command-status-band.tsx:105-108`). Reuse these; do not invent parallel primitives.
- **Reduced motion is already global.** `@media (prefers-reduced-motion: reduce)`
  (`app/globals.css:200`) — drawer/tab transitions must remain usable under it.
- **No focus-trap utility exists** (`grep` for `useFocusTrap`/`FocusTrap`/`inert` finds only
  `command-palette.tsx` + `property-media-panel.tsx`, neither a reusable hook).
- **Repo test convention.** `testv2/*.test.ts` use `node:test` + `node:assert/strict` with
  relative imports (`testv2/projects-workspace-02-tokens.test.ts:1-12`); run via
  `pnpm exec tsx --test`. Runtime `@/` alias resolution under `tsx` is not assumed — new pure
  modules import relative paths only, and component wiring is asserted by **source-scan**
  (the WS-12 AC6 precedent).
- **WS-08/09/10/11/12 code is NOT in this base** (`origin/main@0ac73fb`): no timeline/
  calendar/activity/breadcrumb modules. All of them also edit
  `components/portal/projects-workspace.tsx`; WS-13 must reconcile on landing order (Risk 2).

## Touched surfaces

Files
- `ui/projects/responsive-layout.ts` (NEW, React-free, no `server-only`, no DOM, no clock):
  `WorkspaceLayoutMode`, `WorkspacePanePlacement`, `WorkspaceLayout`, `PROJECTS_BREAKPOINTS`,
  `resolveWorkspaceLayout(width)`, `isDrawerPane(layout, role)`, `WORKSPACE_DRAWER_IDS`,
  `WORKSPACE_REGION_LABELS`.
- `ui/projects/a11y-contract.ts` (NEW, React-free, no DOM, no clock): `WorkspaceRegionRole`,
  `WORKSPACE_ARIA`, `WORKSPACE_KEYBOARD_MAP`, `WORKSPACE_TOUCH_TARGET_MIN`,
  `WORKSPACE_ACCEPTANCE_STATES`, `WorkspaceAcceptanceState`, `WORKSPACE_STATE_CONTRACT`.
- `ui/projects/visual-system.ts` — **additive only**: export `PROJECTS_BREAKPOINTS`
  (`{ medium: 768, large: 1024 }`) and a `PROJECTS_DRAWER_CLASS`/placement class helper if
  needed. Existing exported values/classes are unchanged; the frozen tests must pass verbatim.
- `ui/projects/index.ts` — export the new modules' types/functions/constants.
- `components/portal/ui/use-focus-trap.ts` (NEW, client hook): `useFocusTrap({ active,
  onClose, restoreFocusTo })` returning a container `ref`; traps Tab/Shift+Tab, closes on
  Escape, and restores focus on deactivate.
- `components/portal/projects-workspace.tsx` — realize placement + drawer state + focus
  transfer; convert domain rail and view rail to tabs; convert Work Plan to a treegrid;
  convert the inspector form to announced fields; add live regions; raise touch targets; make
  the New Project popover a labelled dialog. All other views untouched in behavior.
- `app/globals.css` — add `@media (min-width: 768px)` medium grid (navigator + canvas inline,
  inspector as off-canvas drawer) and `<768px` single-column small layout (navigator and
  inspector off-canvas drawers, canvas full width); off-canvas transforms; `focus-visible`
  ring; reduced-motion neutralization. The `@media (min-width: 1024px)` block is unchanged.
- `testv2/projects-workspace-13-a11y-responsive.test.ts` (NEW — story-owned, red→green).

Tables
- None. Read-only path unchanged; no new query.

Routes
- `/portal/projects` only. No new route, no new query param.

Tests
- NEW: `testv2/projects-workspace-13-a11y-responsive.test.ts`.
- Existing, must stay green unmodified:
  `testv2/projects-workspace-02-tokens.test.ts`,
  `testv2/projects-service-projection.test.ts`,
  `testv2/projects-workspace-01-readmodel.test.ts`,
  `testv2/projects-secondary-projection.test.ts`.

## Contract

### C1 — Responsive placement model (pure, total, breakpoint-exact)

`ui/projects/responsive-layout.ts` is pure, total, and deterministic.

```ts
export const PROJECTS_BREAKPOINTS = { medium: 768, large: 1024 } as const

export type WorkspaceLayoutMode = "desktop" | "medium" | "small"
export type WorkspacePanePlacement = "inline" | "drawer"
export type WorkspacePaneRole = "navigator" | "canvas" | "inspector"

export type WorkspaceLayout = {
  mode: WorkspaceLayoutMode
  width: number
  placement: Record<WorkspacePaneRole, WorkspacePanePlacement>
}

export function resolveWorkspaceLayout(width: number): WorkspaceLayout
export function isDrawerPane(layout: WorkspaceLayout, role: WorkspacePaneRole): boolean
```

Placement table (authoritative):

| mode | width | navigator | canvas | inspector |
|------|-------|-----------|--------|-----------|
| desktop | `width >= 1024` | inline | inline | inline |
| medium | `768 <= width < 1024` | inline | inline | drawer |
| small | `width < 768` | drawer | inline | drawer |

- Invariant: `mode` is derived only from `PROJECTS_BREAKPOINTS`; boundaries are inclusive at
  the lower edge (`768` is medium, `1024` is desktop, `767`/`1023` are the tier below).
- Invariant: **canvas is never a drawer**; `navigator` is inline at medium and drawer at
  small; `inspector` is drawer at medium and small.
- Invariant: total — a non-finite, zero, or negative `width` resolves to `small` (fail safe to
  the most constrained layout so a missing measurement can never produce a clipping desktop
  grid). Never throws.
- Invariant: pure — no `window`, no `matchMedia`, no `document`, no clock; the module is
  unit-testable in Node.

### C2 — Semantic contract (pure manifest shared by component and test)

`ui/projects/a11y-contract.ts` encodes the required semantics as data so the component and the
assay cannot drift.

```ts
export type WorkspaceRegionRole =
  | "domain-tabs" | "navigator-tree" | "view-tabs"
  | "work-plan-grid" | "inspector-form" | "live-status" | "alert"

export const WORKSPACE_ARIA: Record<WorkspaceRegionRole, {
  role: string
  label: string
  orientation?: "horizontal" | "vertical"
}>
export const WORKSPACE_KEYBOARD_MAP: {
  tabs: readonly string[]
  tree: readonly string[]
  treegrid: readonly string[]
  drawer: readonly string[]
}
export const WORKSPACE_TOUCH_TARGET_MIN = 44
export const WORKSPACE_REGION_LABELS: { navigator: string; canvas: string; inspector: string }
```

Required semantics (the component must realize every row):

- **Domain rail** — `role="tablist"`, `aria-orientation="vertical"`,
  `aria-label="Project domain"`; each domain button `role="tab"` + `aria-selected` +
  `aria-controls` = the navigator tree panel id; the tree panel is `role="tabpanel"` with
  `aria-labelledby` = the active domain tab id.
- **Navigator tree** — `role="tree"` (provided by `react-arborist`) with an accessible name
  (`aria-label="Project navigator"`); each row `role="treeitem"` with `aria-selected` and
  `aria-expanded` where expandable; each toggle button has a **contextual** name
  (`"Collapse <label>"` / `"Expand <label>"`), not the bare `"Collapse"`/`"Expand"`.
- **View rail** — `role="tablist"`, `aria-orientation="horizontal"`,
  `aria-label="Project workspace views"`; each view button `role="tab"` + `aria-selected` +
  `aria-controls` = its panel id + stable `id`; the content wrapper is `role="tabpanel"` with
  `aria-labelledby` = the active tab id and `tabIndex={0}`. Arrow keys Left/Right/Home/End move
  selection (roving `tabIndex`), with automatic activation.
- **Work Plan** — `role="treegrid"` with `aria-label="Work plan"`; a header `role="row"` of
  `role="columnheader"` cells (Work item / Due / Owner); each work item is a single focusable
  `role="row"` carrying `aria-level`, `aria-expanded` (when it has children), and
  `aria-selected`; its title cell is `role="rowheader"`, due/owner are `role="gridcell"`.
  Keyboard: Up/Down move rows, Right expand, Left collapse, Enter/Space select. Hierarchy is
  never conveyed by indent alone.
- **Inspector form** — every control keeps a programmatic label; helper/error text is linked
  with `aria-describedby`; an invalid control sets `aria-invalid`; the error is rendered with
  `role="alert"` (reuse `PortalFieldError`); the saving/saved region is `role="status"`.
- **Live feedback** — one workspace-level `role="status"` `aria-live="polite"` region announces
  saving / saved / created; failures announce via `role="alert"`. Loading copy uses
  `role="status"`; failure copy uses `role="alert"`; empty/unauthorized use the existing
  `data-state` messages unchanged.
- **Drawers / overlays** — each open drawer and the New Project popover is `role="dialog"` +
  `aria-modal="true"` + `aria-labelledby` (or `aria-label`) and is only mounted while open.

- Invariant: the manifest contains a non-empty `role` and `label` for every region; every
  keyboard group is non-empty; `WORKSPACE_TOUCH_TARGET_MIN === 44`.
- Invariant: the contract module is pure and imports no React/DOM.

### C3 — State contract for the five acceptance states

`WORKSPACE_ACCEPTANCE_STATES` and `WORKSPACE_STATE_CONTRACT` in `a11y-contract.ts` enumerate the
five required states with distinct, non-empty copy and required attributes:

| state | required rendering |
|-------|--------------------|
| populated | the three panes render; no empty/failure message; regions labelled |
| empty | `WorkspaceMessage state="empty"` ("No projects are available in this workspace yet.") |
| saving | `aria-busy="true"` on the form region + `role="status"` "Saving…" |
| error | `role="alert"` failure copy, distinct from `empty` |
| long-content | `PROJECTS_LONG_CONTENT` policies applied: labels truncate, prose wraps, lists scroll; a long title/owner does not overflow its pane |

- Invariant: the five states are pairwise distinct; none reuses the generic
  "not available in this workspace yet" string; the contract reuses `PROJECTS_LONG_CONTENT`
  (never a second long-content policy).
- Invariant: no state fabricates data; `error`/`empty` copy is exactly the existing honest
  copy.

### C4 — Drawer + focus-transfer behavior (client, single-open invariant)

- Local UI state: `navigatorOpen`, `inspectorOpen` (booleans). **At most one drawer is open at
  a time**; opening one closes the other.
- Opening a drawer: record the previously focused element, move focus to the drawer's first
  focusable element (or the drawer container when none), and contain Tab/Shift+Tab inside the
  drawer via `useFocusTrap`.
- Escape closes the open drawer; the backdrop is a `button` labelled "Close …" (palette
  precedent). On close, focus returns to the recorded element if still connected, else to the
  trigger that opened it.
- Selecting a work node at medium/small opens the inspector drawer (closing the navigator
  drawer first). Closing the inspector drawer does **not** clear the controller's selection; a
  persistent "Selected work" trigger reopens it.
- The New Project popover is a labelled dialog with the same trap/Escape/restore behavior and
  initial focus on the name input.
- Invariant: **DOM order stays navigator → canvas → inspector at every breakpoint**; drawers
  are moved visually with CSS transforms, never by reordering the DOM, so screen-reader
  reading order is preserved.
- Invariant: focus is never trapped while no drawer is open; no drawer mounts on the server.

### C5 — SSR safety and placement realization

- Server render must be `window`-free and match the desktop structure. Width is read only
  after mount (effect / `useSyncExternalStore` with a `desktop` server snapshot); the first
  client render equals the server snapshot, so there is no hydration mismatch.
- Placement is realized with CSS breakpoints derived from `PROJECTS_BREAKPOINTS` (Tailwind
  `md:`/`lg:` plus the `@media` blocks in `globals.css`). The component must not branch its
  rendered structure on a width read during render.
- Drawer markup is only mounted while `drawerOpen` (false on the server), so
  `role="dialog"`/`aria-modal` never appear in server HTML.
- Invariant: at `>= 1024px` the DOM, classes, geometry, and surface families are byte-for-byte
  the current approved desktop layout.

### C6 — Touch targets, focus visibility, reduced motion

- Every interactive control has a hit area of at least `44×44` CSS px (`min-h-11` / explicit
  min width). Reuse `portalControlClass` (`min-h-11`) and add `min-h-11` to view tabs and
  domain/rail buttons; icon-only controls get a 44px hit area via padding/min-size.
- Visual size/density is preserved: enlarge hit areas, not glyphs; never change frozen geometry.
- All interactive controls expose a visible `:focus-visible` ring (gold/navy token), never
  removed.
- Drawer/tab transitions are neutralized under `prefers-reduced-motion: reduce`; drawers remain
  fully usable with no transition.
- Invariant: no interactive control in the workspace is below the touch minimum.

## Acceptance (verifiable checks)

| # | Check | Assay command |
|---|-------|---------------|
| AC1 | Responsive table exact: `resolveWorkspaceLayout` maps 767→small, 768→medium, 1023→medium, 1024→desktop; canvas never drawer; navigator inline at medium; non-finite/≤0 → small; never throws; no `window`/`matchMedia`/clock in the module | `pnpm exec tsx --test testv2/projects-workspace-13-a11y-responsive.test.ts` |
| AC2 | ARIA manifest complete: every region has a non-empty role+label; view/domain rails are tabs with orientation; navigator is a named tree; work plan is a named treegrid; keyboard groups non-empty; touch min is 44; contract module is React/DOM-free | `pnpm exec tsx --test testv2/projects-workspace-13-a11y-responsive.test.ts` |
| AC3 | Component wiring (source-scan): view rail `role="tablist"` + `aria-selected` + `aria-controls`; domain rail vertical tablist; tree has `aria-label`; work plan `role="treegrid"` + `columnheader`/`rowheader`/`gridcell` + `aria-level`; inspector errors `role="alert"` + `aria-describedby`/`aria-invalid`; a `role="status"` live region exists; drawers/popover use `role="dialog"`+`aria-modal`+labelled; `useFocusTrap` used; Escape closes; toggle buttons carry contextual labels | `pnpm exec tsx --test testv2/projects-workspace-13-a11y-responsive.test.ts` |
| AC4 | State contract: populated/empty/saving/error/long-content are pairwise distinct and non-empty; none reuses the generic message; long-content reuses `PROJECTS_LONG_CONTENT`; no `Date.now()` in the new pure modules | `pnpm exec tsx --test testv2/projects-workspace-13-a11y-responsive.test.ts` |
| AC5 | Touch/focus/reduced-motion (source-scan): every interactive primitive class includes `min-h-11`; a `:focus-visible` rule exists; drawer transitions are covered by a reduced-motion neutralizer; the `>=1024px` CSS block is unchanged | `pnpm exec tsx --test testv2/projects-workspace-13-a11y-responsive.test.ts` |
| AC6 | Frozen-shape regression: desktop tokens/geometry/surfaces/long-content and the projection read models are unchanged and green | `pnpm exec tsx --test testv2/projects-workspace-02-tokens.test.ts testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts testv2/projects-secondary-projection.test.ts` |
| AC7 | Types + diff clean | `pnpm exec tsc --noEmit` · `git diff --check` |
| AC8 | App builds with the responsive/a11y wiring | `pnpm exec next build --webpack` |
| HG | HUMAN GATE (WS-18): real keyboard traversal of tabs/tree/treegrid/drawers with visible focus; screen-reader announcements of tabs, tree expansion, selection, saving, errors; iPad portrait+landscape with no clipping and practical touch; approved screenshots for populated/empty/saving/error/long-content vs the frozen desktop reference | HUMAN GATE — not machine-verifiable (no browser tooling in this repo) |

## Assay plan (SCOPED — run exactly this)

```sh
pnpm exec tsx --test testv2/projects-workspace-13-a11y-responsive.test.ts
pnpm exec tsx --test testv2/projects-workspace-02-tokens.test.ts testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts testv2/projects-secondary-projection.test.ts
pnpm exec tsc --noEmit
git diff --check
pnpm exec next build --webpack
```

Forbidden: `pnpm test`, `pnpm test:persistence`, `pnpm test:engine`, `pnpm test:app`,
npm/yarn test, any multi-file workflow glob. If `next build` cannot run, record the blocker
and escalate — do not fall back to the full harness.

## Risks (what could invalidate this plan)

1. **No browser tooling (HIGH).** Keyboard traversal, screen-reader announcements, touch
   ergonomics, and pixel comparison cannot be machine-verified here; the pure manifest and
   source-scan are *proxies*, not proof. Mitigation: the single `a11y-contract.ts` is the
   shared source of truth for component and test, and HG is the explicit acceptance. Do not
   claim HG is satisfied by green tests.
2. **Shared-surface collision with WS-08/09/10/11/12 (HIGH).** All edit
   `components/portal/projects-workspace.tsx` (and WS-11/12 also `ui/projects/*`, page.tsx).
   WS-13 must rebase onto whichever lands first and apply the semantics to **every view
   present** (timeline/calendar/documents/activity/breadcrumbs), without reverting their
   additions. If a view is added after WS-13 lands, that is a follow-up, not a WS-13 widening.
3. **Frozen desktop geometry must not move (HIGH).** `projects-workspace-02-tokens.test.ts`
   locks the tracks/surfaces/policies. Any new CSS must be scoped `@media` and must leave the
   `>=1024px` block unchanged; `PROJECTS_GEOMETRY.columns.*` values are not edited.
4. **`react-arborist` role contract unverified (MED).** The library may render
   `role="tree"`/`treeitem` already; if it does not, add the accessible name and document the
   gap rather than forking the library. Verify before writing around it.
5. **Focus trap vs virtualized tree (MED).** Trapping focus at the drawer container (not
   per-row) is required; react-arborist virtualizes rows, so a per-row trap can lose focus on
   scroll. Keep the trap container-level and test Escape/restore.
6. **Two drawers + selection (MED).** Enforce the single-open invariant and that closing the
   inspector does not clear selection; otherwise a focus fight or a lost selection appears.
7. **Hydration mismatch (MED).** Reading width during render would desync server/client HTML
   and roles. Width is read only post-mount; drawers mount only when open. Do not
   conditionally render the inline panes on a width read.
8. **Touch inflation changes density (LOW).** Raise hit areas only; never enlarge glyphs or
   alter the approved desktop visual balance.
9. **Treegrid conversion size (MED).** Work Plan goes from nested buttons to a `treegrid`;
   this is the largest single edit. If the Lead judges it oversized for one Smith unit, it may
   be SPLIT as its own child, but it must land within WS-13's scope.
10. **Pre-existing `GuideItem` tsc noise / `next build` cost (LOW).** Known; do not broaden
    scope to fix it. If the build cannot run, record the blocker rather than skipping silently.

## Release obligations

- migrationRequired: false (presentation/semantics only; no schema, no new read).
- derivedRefreshRequired: false.
- deploymentRequired: true (Vercel UI on `/portal/projects`; `globals.css` + component).
- DEV/PROD schema/data: untouched.

## Handoff to Lead

Build C1 (`responsive-layout.ts`) and C2/C3 (`a11y-contract.ts`) FIRST — pure and fully
unit-tested, so Assay is runnable before any JSX. Then C5/C6 (CSS media queries + touch/focus/
reduced-motion), then C2 semantics (domain tabs, view tabs, navigator label, treegrid), then
C4 (drawer state + `useFocusTrap` + New Project dialog), then the live regions and inspector
field announcements. Extract ALL decision logic into the two pure modules; the component only
maps state to placement/DOM/ARIA. Do not modify `PROJECTS_GEOMETRY` values, the `>=1024px` CSS
block, `plan.*`, `getActivityFeed`, `lib/portal-time.ts`, the global `CommandPalette`, or the
other views' behavior. Reconcile on the WS-08/09/10/11/12 landing order and apply semantics to
every view present. Report exact files changed and the SCOPED Assay result. No commit/push from
the architect node.

## Loop decision (planner)

intent: grow (new slice from PROJECTS-WORKSPACE-12). One packet, one scope. No repair folded
in. Ready for Chris to flip; Forge A1 is the only writer.
