# PROJECTS-WORKSPACE-12 — Shareable context: local search, canonical breadcrumbs, URL-addressable selection (ARCHITECT CONTRACT)

Lane: architect. Design truth only. No implementation, no commit, no DEV/PROD write.

## Loop
intent: grow
parent_run: PROJECTS-WORKSPACE-11 (activity projection) — batch 1
loop: 1/3

## Goal (one line)

Make a project's context recoverable and shareable by encoding the selected
domain/pole/project/work-item/tab in canonical URL state on `/portal/projects`,
reconciling it against authorized data on load and on back/forward, rendering a
canonical derived breadcrumb trail, and keeping the local (Pane 1) search scope
explicitly distinct from the global (⌘K) CommandPalette.

## Non-goals (explicit)

- No schema change, no migration, no DDL, no new DB read, no new route. The page is
  already `force-dynamic` and reads the same authorized payload.
- No change to the global `CommandPalette` catalog, props, or mount. It stays the
  GLOBAL surface (surfaces + people + deals). WS-12 does not thread projects into
  `OperatingShell` (Risk 8).
- No second navigation/selection store. The MVI `ProjectsWorkspaceController` remains
  the single writer of selection; URL is a serialization of that state, never a
  parallel authority.
- No persistence of the Pane 1 search text in the URL (it is ephemeral; keeps history
  clean). "Local search" is the existing Pane 1 tree filter.
- No change to `plan.*` shapes, `getActivityFeed`, `lib/portal-time.ts`, the view rail
  set, the three-pane geometry, or any other view (Timeline/Calendar/Documents/
  Financials/Activity) beyond the header breadcrumb line.
- No provider/OAuth read; no listing special-casing; SCOPED only; no push/merge/deploy;
  no DEV/PROD write.

## Verified starting evidence (grounding)

- **The controller ignores any external selection and always defaults.** `load()` picks
  `firstPole`/`firstProject` and `defaultFocus` with no input beyond `activeDomain`
  (`ui/projects/projects-controller.ts:149-175`). There is no URL read anywhere on the
  Projects route.
- **Selection is currently lost on `router.refresh()`.** `source`/`controller` are
  `useMemo`-recreated whenever `initialData` changes
  (`components/portal/projects-workspace.tsx:827-836`); after any refresh (status save,
  WBS save) the controller is new and `projects.load` resets to the first Pole/Project.
  This is the concrete "context is not recoverable" defect WS-12 fixes.
- **The page reads no `searchParams`.** `app/portal/projects/page.tsx:25` sets
  `dynamic = "force-dynamic"`; the default export takes no props and renders
  `<ProjectsWorkspace initialData loadError />` (`:146-149`). The repo's Next 16
  async-searchParams pattern is `app/portal/storyboard/page.tsx:18-20`
  (`searchParams: Promise<...>` then `await`).
- **The model already holds the exact URL-worthy fields.** `ProjectsWorkspacePageModel`
  has `activeDomain`, `selectedPoleId`, `selectedProjectId`, `selectedNodeId`,
  `activeView` (`ui/projects/model.ts:131-142`); the intent map has the matching
  selection ops (`:159-168`).
- **Selection identity is stable ids, not display names.** Arborist composite ids are
  `poleId`, `poleId::projectId`, `parent::workNodeId`
  (`ui/projects/tree-projection.ts:40-120`); dispatch uses the real model ids
  (`components/portal/projects-workspace.tsx:889-908`). URL state therefore uses real
  ids and never names.
- **Pane 1 owns the local search.** The "Find work…" input filters the arborist tree by
  `searchText` within the active domain (`components/portal/projects-workspace.tsx:375-383`,
  `:356-360`). The global palette is a separate ⌘K overlay mounted in the shell
  (`components/portal/command-palette.tsx:22-92`, `components/portal/operating-shell.tsx:170`).
- **The header already has a pseudo-breadcrumb.** `ProjectHeader` renders
  `pole.label › project.kind` (`components/portal/projects-workspace.tsx:527-529`) — no
  domain, no work item, not derived from a projection, not addressable.
- **Runtime versions.** Next `16.3.0`, React `^19` (`package.json:88,92`). Next 16
  supports native `window.history.pushState`/`replaceState` integration with App Router.
- **Test convention.** `testv2/*.test.ts` use `node:test` + `node:assert/strict` with
  relative imports (`testv2/projects-workspace-01-readmodel.test.ts:1-8`); run via
  `pnpm exec tsx --test` (WS-11 Assay). Runtime `@/` alias resolution under `tsx` is not
  assumed — new pure modules use relative imports only (Risk 4).
- **WS-08/09/11 code is NOT in this base** (`origin/main@edcf3d8`): no timeline/calendar
  projection modules. Keep WS-12 self-contained and reconcile on landing order (Risk 6).

## Touched surfaces

Files
- `ui/projects/url-state.ts` (NEW, React-free, no `server-only`, no I/O, no clock):
  `ProjectWorkspaceUrlState`, `PROJECTS_URL_PARAMS`, `DEFAULT_PROJECTS_VIEW`,
  `parseProjectsUrlState`, `serializeProjectsUrlState`, `normalizeProjectsUrlState`,
  `urlStateFromSelection`, `isProjectDomainKey`, `isProjectWorkspaceView`.
- `ui/projects/selection-reconcile.ts` (NEW, React-free, pure): `ResolvedProjectSelection`,
  `defaultFocusNode`, `findWorkNodeInProject`, `reconcileProjectSelection`. Moves the
  controller's `focusNodeId`/`defaultFocus` here as the single source (controller imports it).
- `ui/projects/breadcrumb-projection.ts` (NEW, React-free, pure): `ProjectBreadcrumb`,
  `ProjectBreadcrumbKind`, `buildProjectBreadcrumbs`.
- `ui/projects/model.ts` — **additive**: `"projects.restoreFromUrl"` intent in
  `ProjectsWorkspaceIntentMap` (request `ProjectWorkspaceUrlState`, response `void`).
  Existing intents unchanged.
- `ui/projects/projects-controller.ts` — `load()` reconciles via
  `reconcileProjectSelection(data, context.snapshot())` instead of hard-defaulting; add
  the `projects.restoreFromUrl` operation (reconcile against loaded data, or store the
  requested fields when data is not yet loaded so `load()` applies them). Remove the now
  relocated `focusNodeId`/`defaultFocus`.
- `ui/projects/index.ts` — export the new modules' types/functions.
- `components/portal/projects-workspace.tsx` — accept `initialUrlState`; seed the
  controller's initial model from it; add the URL write effect + `popstate` read effect;
  render `<ProjectBreadcrumbs>` in the Pane 2 header; make the Pane 1 search scope
  explicit (visible/aria label). All other views untouched.
- `app/portal/projects/page.tsx` — accept and `await` `searchParams`, parse with
  `parseProjectsUrlState`, pass `initialUrlState` to `<ProjectsWorkspace>`. No change to
  the service reads or the failure/authorization branches.
- `testv2/projects-workspace-12-url-state.test.ts` (NEW — story-owned, red→green).

Tables
- None. Read-only path unchanged; no new query.

Routes
- `/portal/projects` only. Adds query params (`domain`,`pole`,`project`,`node`,`view`);
  no new route segment.

Tests
- NEW: `testv2/projects-workspace-12-url-state.test.ts`.
- Existing, must stay green: `testv2/projects-service-projection.test.ts`,
  `testv2/projects-workspace-01-readmodel.test.ts`,
  `testv2/projects-secondary-projection.test.ts`.

## Contract

### C1 — Canonical URL state (pure, deterministic, no clock, no I/O)

`ui/projects/url-state.ts` is pure and total.

```ts
import type { ProjectDomainKey, ProjectWorkspaceView } from "./model"

export type ProjectWorkspaceUrlState = {
  domain: ProjectDomainKey | null
  pole: string | null
  project: string | null
  node: string | null
  view: ProjectWorkspaceView | null
}

export const PROJECTS_URL_PARAMS = {
  domain: "domain", pole: "pole", project: "project", node: "node", view: "view",
} as const
export const DEFAULT_PROJECTS_VIEW: ProjectWorkspaceView = "work-plan"

export function isProjectDomainKey(v: string | null | undefined): v is ProjectDomainKey
export function isProjectWorkspaceView(v: string | null | undefined): v is ProjectWorkspaceView

export function parseProjectsUrlState(
  input: string | URLSearchParams | Readonly<Record<string, string | string[] | undefined>>,
): ProjectWorkspaceUrlState

export function serializeProjectsUrlState(state: Partial<ProjectWorkspaceUrlState>): string

export function normalizeProjectsUrlState(
  state: Partial<ProjectWorkspaceUrlState>,
): ProjectWorkspaceUrlState

export function urlStateFromSelection(selection: {
  activeDomain: ProjectDomainKey
  selectedPoleId: string | null
  selectedProjectId: string | null
  selectedNodeId: string | null
  activeView: ProjectWorkspaceView
}): ProjectWorkspaceUrlState
```

- `parseProjectsUrlState` accepts a raw `"a=b&c=d"` string (leading `?` tolerated), a
  `URLSearchParams`, or a Next `searchParams` record; the first value wins for repeated
  keys; unknown keys are ignored.
- Validation: `domain` kept only when `isProjectDomainKey`; `view` kept only when
  `isProjectWorkspaceView`; `pole`/`project`/`node` kept as non-empty trimmed strings
  (their existence is validated later against authorized data, never here).
- Serialization is canonical: fixed key order `domain, pole, project, node, view`;
  `view` omitted when `null` or `DEFAULT_PROJECTS_VIEW`; each other field omitted when
  `null`/empty; `URLSearchParams` encoding; returns without a leading `?`.
- Invariant: pure — same input → deep-equal output; never throws on malformed input.
- Invariant: round-trip stable — `parse(serialize(s))` deep-equals
  `normalize(s)` for every state; serialization emits only values `parse` accepts.
- Invariant: no `Date.now()`/`new Date()`; the module never reads the machine clock or
  `window`.

### C2 — Reconciliation against authorized data (pure, total, non-widening)

`ui/projects/selection-reconcile.ts` is pure and total.

```ts
import type {
  ProjectsWorkspaceData, ProjectDomainKey, ProjectWorkNode, ProjectWorkspaceView,
} from "./model"
import type { ProjectWorkspaceUrlState } from "./url-state"

export type ResolvedProjectSelection = {
  activeDomain: ProjectDomainKey
  selectedPoleId: string | null
  selectedProjectId: string | null
  selectedNodeId: string | null
  activeView: ProjectWorkspaceView
  expandedPoleIds: string[]
}

export function defaultFocusNode(
  project: { workNodes: ProjectWorkNode[] } | null | undefined,
): string | null

export function findWorkNodeInProject(
  project: { workNodes: ProjectWorkNode[] } | null | undefined,
  nodeId: string | null,
): ProjectWorkNode | null

export function reconcileProjectSelection(
  data: ProjectsWorkspaceData,
  requested: Partial<ProjectWorkspaceUrlState>,
): ResolvedProjectSelection
```

Resolution order (deterministic):

1. `activeView = requested.view ?? DEFAULT_PROJECTS_VIEW`.
2. **Pole**: `requested.pole` if a pole with that id exists anywhere in `data`; else the
   first pole in `requested.domain` (when valid and present); else the first pole in
   `data.poles`; else `null`.
3. `activeDomain` = the resolved pole's `domain`; when no pole resolves, `requested.domain`
   if valid, else `data.domains[0]?.key ?? "properties"`.
4. **Project**: `requested.project` if present in the resolved pole's `projects`; else
   `pole.projects[0] ?? null`.
5. **Node**: `requested.node` if present anywhere in the resolved project's `workNodes`
   tree; else `defaultFocusNode(project)` (deepest waiting/in-progress descendant, else
   the first node, else `null`).
6. `expandedPoleIds = resolved pole ? [pole.id] : []`.

- Invariant: total — never throws; malformed/empty `data` yields a coherent empty result.
- Invariant: internally consistent — a resolved `selectedProjectId` always belongs to the
  resolved pole; `selectedNodeId` always belongs to the resolved project; `activeDomain`
  always equals the resolved pole's domain.
- Invariant: **non-widening** — every id returned exists in the provided `data`; the
  function cannot synthesize, guess, or surface an id that is not already present. A URL
  id that is unknown or not in the caller's authorized payload is dropped, not reported.
- Invariant: `defaultFocusNode`/`findWorkNodeInProject` reproduce the controller's current
  focus semantics exactly (they replace `projects-controller.ts:17-29`).

### C3 — Controller integration (single writer; restores on load and on navigation)

- `load()` computes `reconcileProjectSelection(data, context.snapshot())` and applies the
  resolved fields. Because the snapshot is seeded from URL state (C4), the requested
  selection survives load; with no request the behavior is identical to today.
- New intent `"projects.restoreFromUrl"` (`ui/projects/model.ts`, request
  `ProjectWorkspaceUrlState`):
  - if `data` is loaded → `context.update(model => ({ ...model, ...reconcile(data, request) }))`;
  - if `data` is not yet loaded → store the requested fields on the model
    (`activeDomain`/`selectedPoleId`/`selectedProjectId`/`selectedNodeId`/`activeView`) so
    the pending `load()` reconciles them.
- The existing `projects.selectDomain/selectPole/selectProject/selectNode/selectView`
  operations are unchanged and remain the only user-driven writers.
- Invariant: URL is never a second source of truth — it is parsed to a request, reconciled
  against authorized data, and the reconciled model is what gets serialized back.
- Invariant: an invalid/unknown/unauthorized id produces the deterministic default (C2)
  with **no error state, no banner, and no leak** that the id exists.

### C4 — Page-level URL read (server) and client initialization

- `app/portal/projects/page.tsx`:
  ```ts
  export default async function ProjectsPage({
    searchParams,
  }: { searchParams: Promise<{ [key: string]: string | string[] | undefined }> }) {
    const initialUrlState = parseProjectsUrlState(await searchParams)
    const result = await loadRealProjectsData()
    return <ProjectsWorkspace initialData={result.data} loadError={result.error}
                              initialUrlState={initialUrlState} />
  }
  ```
  The service reads, authorization branches, and failure states are untouched.
- `ProjectsWorkspace` seeds the controller once from `initialUrlState`:
  `new ProjectsWorkspaceController(source, { ...INITIAL_PROJECTS_WORKSPACE_MODEL, ...modelPatch(initialUrlState) })`,
  where the patch maps `domain→activeDomain`, `pole→selectedPoleId`,
  `project→selectedProjectId`, `node→selectedNodeId`, `view→activeView`.
- On mount, and whenever the controller is recreated (e.g. `router.refresh()`), a client
  effect reads `window.location.search` and dispatches `projects.restoreFromUrl`; this is
  the durable fix for "selection lost on refresh" and makes the address bar authoritative
  for recovery. Server rendering stays `window`-free.
- Invariant: SSR output does not read `window`; the initial model comes from the awaited
  server `searchParams`.

### C5 — URL write + back/forward (client, no feedback loop)

- Write effect (in `ProjectsWorkspace`): when `model.data` is loaded, compute
  `desired = serializeProjectsUrlState(urlStateFromSelection(model))` and compare to
  `window.location.search` (leading `?` stripped). If different, update the URL with the
  native History API (Next 16 integrates it with App Router):
  - the first normalization after a load (e.g. an invalid URL fell back) uses
    `replaceState` so a bad link is corrected without a junk history entry;
  - subsequent discrete selection changes (`domain`/`pole`/`project`/`node`/`view`) use
    `pushState` so Back/Forward traverse selections.
- Read effect: a `popstate` listener parses `window.location.search` and dispatches
  `projects.restoreFromUrl`. It is the only URL→model path besides initial load.
- Invariant: no loop — a programmatic write leaves the URL equal to `desired`, so the
  resulting reconcile is a no-op and no further write occurs; `popstate` fires only on
  genuine Back/Forward.
- Invariant: the local search text (`model.query`) never triggers a URL write and never
  enters history; typing in Pane 1 does not spam Back.
- Invariant: Back/Forward across selection changes restores the prior selection
  (pole/project/node/tab) without a full reload.

### C6 — Canonical breadcrumbs (pure, derived, no hardcoding)

`ui/projects/breadcrumb-projection.ts`:

```ts
import type { ProjectDomain, ProjectDomainKey, ProjectPole, ProjectPlan, ProjectWorkNode } from "./model"

export type ProjectBreadcrumbKind = "root" | "domain" | "pole" | "project" | "node"
export type ProjectBreadcrumb = { id: string; kind: ProjectBreadcrumbKind; label: string }

export function buildProjectBreadcrumbs(input: {
  domains: readonly ProjectDomain[]
  activeDomain: ProjectDomainKey
  pole: ProjectPole | null
  project: ProjectPlan | null
  node: ProjectWorkNode | null
}): ProjectBreadcrumb[]
```

- Trail: `root` "Projects" (static) → `domain` (label from `domains` for `activeDomain`;
  omitted when the key has no matching definition — never invented) → `pole` (label) →
  `project` (title) → `node` (title). Ancestor crumbs appear only when their object is
  present; the array is never padded.
- Ids: `root`, `domain:<key>`, `pole:<pole.id>`, `project:<project.id>`, `node:<node.id>`
  — stable, unique, derived from canonical identity (never display-name equality).
- Invariant: pure and deterministic; labels come only from the loaded model; no listing,
  project, or pole is special-cased.
- Rendering: a `ProjectBreadcrumbs` component in the Pane 2 header **replaces** the
  `pole.label › project.kind` eyebrow (`components/portal/projects-workspace.tsx:527-529`).
  Each non-final segment is a button dispatching the matching selection op
  (`domain→selectDomain`, `pole→selectPole`, `project→selectProject`, `node→selectNode`);
  the final segment is non-interactive with `aria-current="page"`. Interactive targets use
  ≈44–48px min height for iPad. Title/status/progress block is untouched.

### C7 — Search scope clarity (global vs local)

- Pane 1's input stays the LOCAL search: it filters only the current domain's tree by
  `searchText`. Make the scope explicit and non-ambiguous — a visible scope affordance
  (e.g. the active domain label already above it) plus an explicit
  `aria-label="Search this workspace"` and a placeholder that names the scope
  (e.g. "Find work in this domain…"). No behavior change to filtering.
- The global `CommandPalette` (⌘K) is unchanged and remains the GLOBAL surface; it is not
  imported or rendered inside `projects-workspace.tsx`.
- Invariant: the two search affordances are textually/structurally distinct; the local
  one is scoped by domain and never performs a cross-surface query.

## Acceptance (verifiable checks)

| # | Check | Assay command |
|---|-------|---------------|
| AC1 | URL round-trip is canonical: `parse(serialize(s))` deep-equals `normalize(s)`; defaults (`view=work-plan`) omitted; key order `domain,pole,project,node,view`; unknown params ignored; malformed `domain`/`view` dropped, not thrown | `pnpm exec tsx --test testv2/projects-workspace-12-url-state.test.ts` |
| AC2 | Authorized selection restores: a requested domain/pole/project/node/view present in the fixture reconciles to exactly those ids (including a nested work node) | `pnpm exec tsx --test testv2/projects-workspace-12-url-state.test.ts` |
| AC3 | Invalid/unauthorized IDs fall back neutrally: unknown pole/project/node/domain/view and empty data yield a coherent default (first pole/project, `work-plan`) with no throw and no error field | `pnpm exec tsx --test testv2/projects-workspace-12-url-state.test.ts` |
| AC4 | Non-widening: over the fixture, every id `reconcileProjectSelection` returns is present in the input `data`; a cross-domain pole/project request never leaks another project's nodes | `pnpm exec tsx --test testv2/projects-workspace-12-url-state.test.ts` |
| AC5 | Breadcrumbs are canonical and derived: full trail root>domain>pole>project>node with model labels; partial context yields a shorter trail; no match for the domain key omits the domain crumb; no hardcoded listing string | `pnpm exec tsx --test testv2/projects-workspace-12-url-state.test.ts` |
| AC6 | Scope clarity + wiring (source-scan): Pane 1 search carries a local scope label; `projects-workspace.tsx` does not import/render `CommandPalette`; the controller declares `projects.restoreFromUrl` and `load()` calls `reconcileProjectSelection`; `page.tsx` awaits `searchParams` and parses via `parseProjectsUrlState` | `pnpm exec tsx --test testv2/projects-workspace-12-url-state.test.ts` |
| AC7 | Frozen-shape regression: projection tests unchanged and green | `pnpm exec tsx --test testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts testv2/projects-secondary-projection.test.ts` |
| AC8 | Types + diff clean | `pnpm exec tsc --noEmit` · `git diff --check` |
| AC9 | App builds with the URL wiring + breadcrumbs | `pnpm exec next build --webpack` |
| HG | Human: copy a selection URL into a fresh tab → same domain/pole/project/tab/work item; Back/Forward traverses selections; a bad/foreign id URL lands neutrally and self-corrects; breadcrumbs navigate; local vs global search is obvious; iPad targets | HUMAN GATE — not machine-verifiable |

## Assay plan (SCOPED — run exactly this)

```sh
pnpm exec tsx --test testv2/projects-workspace-12-url-state.test.ts
pnpm exec tsx --test testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts testv2/projects-secondary-projection.test.ts
pnpm exec tsc --noEmit
git diff --check
pnpm exec next build --webpack
```

Forbidden: `pnpm test`, `pnpm test:persistence`, `pnpm test:engine`, `pnpm test:app`,
npm/yarn test, any multi-file workflow glob. If `next build` cannot run, record the
blocker and escalate — do not fall back to the full harness.

## Risks (what could invalidate this plan)

1. **History API ↔ App Router ↔ `router.refresh()` (MED).** Next 16 supports native
   `pushState`/`replaceState`, but `router.refresh()` re-fetching the current URL with
   client-pushed params must be confirmed. Mitigation: the mount/`popstate` effect always
   re-reads `window.location.search` and reconciles, so recovery does not depend on the
   server seeing the param; if refresh proves unreliable, fall back to `router.replace(url,
   { scroll: false })` (heavier RSC round-trip) rather than break the invariant. HG
   confirms behavior.
2. **Controller recreation resets selection (MED).** `initialData` identity changes on
   refresh (`projects-workspace.tsx:827-836`), recreating source/controller. WS-12's
   URL-seeded initial model + mount reconcile must preserve selection; do NOT "fix" this by
   mutating `initialData` or by holding the controller across refreshes (that would stale
   the source). Verify a status/WBS save does not drop the selection.
3. **Param canonicality drift (LOW).** Verbose-but-explicit params (`domain` redundant when
   `pole` present) are acceptable; do not add data-dependent omission (serialization must
   stay pure and data-free). Keep `view=work-plan` omitted and ids stable.
4. **Alias resolution in tests (LOW).** New pure modules must import only relative paths
   (`./model`, `./url-state`) so `tsx --test` resolves them without tsconfig-paths. Do not
   import the controller in the test (it uses `@/ui/runtime`); assert its wiring by
   source-scan (AC6).
5. **History noise (LOW).** Only discrete selections push; the initial normalization
   replaces; `query` never writes. Do not push on every render or on `model.query`.
6. **Shared-surface collision with WS-08/09/11 (MED).** All touch
   `components/portal/projects-workspace.tsx`, `app/portal/projects/page.tsx`,
   `ui/projects/model.ts`, and `ui/projects/index.ts`. WS-12's model change is one additive
   intent and its header change is the eyebrow line only; the Lead must rebase/reconcile on
   whichever lands first.
7. **Breadcrumb replaces the header eyebrow (LOW).** Keep the title/status/progress/next-
   action block intact; only the `pole › kind` line is replaced. Preserve iPad touch sizing.
8. **"Reuse global CommandPalette" ambiguity (MED).** Decision: the global palette is NOT
   modified and projects are NOT added to it (that needs project data threaded through
   `OperatingShell`, a separate story). WS-12 satisfies "scopes are clear" by labeling the
   local Pane 1 search and leaving the ⌘K palette as the global surface. If a reviewer
   requires project entries in the palette, raise it as a FOLLOW-UP story — do not widen
   WS-12.
9. **Node auto-focus normalizes the URL (LOW).** A `project`-only link gains a `node` once
   `defaultFocusNode` selects one. This is honest (the inspector shows it) and canonical;
   document it in the handoff rather than suppressing focus.
10. **Pre-existing `GuideItem` tsc noise (LOW).** Known; do not broaden scope to fix it.

## Release obligations

- migrationRequired: false (URL/breadcrumb/selection only; no schema, no new read).
- derivedRefreshRequired: false.
- deploymentRequired: true (Vercel UI + `/portal/projects` page loader).
- DEV/PROD schema/data: untouched.

## Handoff to Lead

Build C1 (`url-state.ts`) FIRST — pure, fully unit-tested, so Assay is runnable before any
JSX. Then C2 (`selection-reconcile.ts`, moving `focusNodeId`/`defaultFocus` out of the
controller) and C6 (`breadcrumb-projection.ts`). Then C3 (controller: `load()` reconcile +
`restoreFromUrl`), C4 (page `searchParams` + seeded initial model + mount reconcile), C5
(URL write + `popstate`, with the no-loop guards), C7 (scope labels + `ProjectBreadcrumbs`
render). Extract ALL decision logic into the three pure modules; the component only maps
state to URL/DOM. Do not modify the global palette, `plan.*`, `getActivityFeed`,
`lib/portal-time.ts`, the view rail, or the other views. Report exact files changed and the
SCOPED Assay result. No commit/push from the architect node.

## Loop decision (planner)

intent: grow (new slice from PROJECTS-WORKSPACE-11). One packet, one scope. No repair
folded in. Ready for Chris to flip; Forge A1 is the only writer.
