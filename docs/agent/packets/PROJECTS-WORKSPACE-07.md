# PROJECTS-WORKSPACE-07 — Work-item domain actions and linked-record navigation (ARCHITECT CONTRACT)

Lane: architect. Design truth only. No implementation, no commit, no DEV/PROD write.

## Loop
intent: grow
parent_run: PROJECTS-WORKSPACE-06 (selected-work inspector and safe edits)
loop: 1/3

## Goal (one line)

Give every listing-playbook work item typed, server-resolved actions that open — or
start/continue — the correct Forms, signature, Cabinet, Marketing, and Accounting
record, or return an explicit unavailable reason, with authorization enforced and a
replay-safe domain completion projection.

## Non-goals (explicit)

- No schema change, no migration, no new table/column. Work type is DERIVED, not persisted (see Risks 1).
- No change to WS-06 inspector fields, Pane 1/2 geometry, tree, projection ordering, or `project` writes.
- No new service domain; `ProjectService` owns projects, `WbsService` owns work items — unchanged.
- No change to the five capability surfaces' own routes/behavior; this story only navigates INTO them.
- No re-implementation of form/signature/media/syndication/accounting logic; start/continue DELEGATES to existing server actions.
- No free-text assignee, no WhatsApp/person-identity work.
- No listing special-casing (Casa Luar or any listing).
- No full regression (SCOPED only). No push/merge/deploy. No DEV/PROD schema or business-data write.

## Verified starting evidence (grounding)

- Playbook nodes: `services/project/playbooks.ts:19` — `LISTING_ONBOARDING_V1` keys
  `parties, property, agreement, signature(parent agreement), media, marketing, accounting`.
- Instantiation: `services/project/project-service.ts:97` (`project.instantiate`) creates
  WBS items with id `${project.id}-${node.key}` and calls `wbs.create` — **it sets no `entity` anchor** (`:138`).
- WBS item carries `entity {type: person|property|contract|deal, id}` only (`services/wbs/types.ts:6`);
  `wbs_item` columns are fixed in `db/migrations/124_wbs_project_item.sql`; there is **no playbook-key/work_type column**.
- Project carries `personId/propertyId/contractId/playbookId/playbookVersion` (`db/migrations/141_project_playbook_identity.sql`).
- Current actions are generic strings, not typed/records: `nodeActions` in
  `ui/projects/service-projection.ts:141` maps entity type → `"Open client|property|contract|deal"` only;
  rendered at `components/portal/projects-workspace.tsx:796`.
- Listing form evidence seam: `lib/forms/listing-canonical-binding.ts:75` (`latestListingEvidence`,
  `template_id = 'LISTING-01'`, matches person via `person_id` or deal participant).
- Form instances: `db/document-form-instance.ts:284` (`listFormInstances`), `:123` (`getFormInstance`).
  Route: `app/portal/forms/[formId]/page.tsx`; the FormEditorSurface already surfaces the signature
  request (`components/portal/forms/form-editor-surface.tsx:142`). Signature start = `sendFormForSignatureAction`
  (`app/portal/forms/actions.ts`, `actions-core.ts:306`).
- Active signature request read: `db/signature-request.ts:568` (`getActiveSignatureRequestForDocument`),
  `:586` (`listSignatureRequestsByDocument`).
- Cabinet: `db/transaction-document.ts:533` (`listIssuedDocuments`), `db/media-admin.ts:42` (`getMediaAdmin`).
  Routes: `app/portal/documents`, `app/portal/property-media`, `app/portal/media-admin`.
- Marketing/syndication is property-scoped: `db/syndication.ts` `listPlacements` `:176`,
  `listSightings(propertyId?)` `:422`, `getMarketingDashboard` `:202`. Route `app/portal/marketing`.
- Accounting: `db/accounting.ts` `getReceivables` `:123`, `getExpenses` `:145`, `getAccountingDashboard` `:166`.
  Route `app/portal/accounting`.
- Authorization: `AuthorizationService` + `StaticAuthorizationPolicyProvider`
  (`services/entitlement/authorization-service.ts:49,134`); route guards use action codes such as
  `deal.read` / `project.read` / `portal.read` (grep of `app/portal/**` `resolvePortalAccess`).
- Error capture: `captureServerError` / `withServerErrorCapture` (`lib/server-error-capture.ts`,
  `lib/error-capture-seam.ts`); expected denials/not-found are audited control flow, never error rows (AGENTS).
- Precedent for a pure, React-free logic module: `ui/projects/inspector-form.ts` (WS-06).

## Touched surfaces

Files
- `ui/projects/work-actions.ts` (NEW, React-free, no `server-only`) — the typed registry, the work-type
  derivation, the pure capability-planning + completion projection, and the shared result types.
- `app/portal/projects/action-resolution.ts` (NEW, `import 'server-only'`) — server-side relationship +
  authorization resolution over injected read/authorize ports; batches reads by anchor set.
- `ui/projects/model.ts` — extend `ProjectWorkNode` with `workActions?: ResolvedWorkAction[]` and
  `linkedStatus?: ProjectWorkStatus`; re-export the action types.
- `ui/projects/service-projection.ts` — delete the string `nodeActions`; accept resolved actions and thread
  `linkedStatus`; keep projection pure.
- `ui/projects/index.ts` — export `work-actions`.
- `app/portal/projects/page.tsx` — call `resolveWorkActions(...)`, thread results into
  `mapRealProjectsToWorkspace`; capture failures (`captureServerError`), degrade to unavailable — never 500.
- `components/portal/projects-workspace.tsx` — Pane 3 renders typed actions: `navigate` = `<Link href>`;
  `start`/`continue` = existing server actions; `unavailable` = explanatory text (`role="status"`).
- `app/portal/projects/actions.ts` — only if a thin start/continue dispatcher is needed; it must delegate to
  the existing `createFormAction` / `sendFormForSignatureAction` (no duplicated capability logic).

Tables
- Read-only: `project`, `wbs_item`, `document_form_instance`, `document_form_participant`, `deal`, `deal_participant`,
  `signature_request`, `transaction_document`, `media`/`property_media`, `syndication`/placement tables,
  `account_receivable`/`account_expense`, `property`, `person`/`mv_client_directory`, `contract`. No DDL.

Routes
- `/portal/projects` (server page). Navigation targets only (no new routes):
  `/portal/forms/[formId]`, `/portal/documents`, `/portal/property-media`, `/portal/media-admin`,
  `/portal/marketing`, `/portal/accounting`, `/portal/clients`, `/portal/property-admin`, `/portal/deals`,
  and `/portal/workflows/[instanceId]` only when a canonical process instance is already linked.

Tests
- `testv2/projects-workspace-07-actions.test.ts` (NEW — story-owned, red→green),
  `testv2/projects-service-projection.test.ts`, `testv2/projects-workspace-01-readmodel.test.ts`,
  `testv2/projects-workspace-06-inspector.test.ts`.

## Contract

### C1 — Typed action registry (pure, keyed by work type / entity / capability)

`ui/projects/work-actions.ts` is the single source of truth. No I/O, no clock, no randomness.

```ts
export type WorkType =
  | 'parties' | 'property' | 'agreement' | 'signature' | 'media' | 'marketing' | 'accounting' | 'adhoc'
export type WorkActionCapability = 'record' | 'form' | 'signature' | 'cabinet' | 'marketing' | 'accounting'
export type WorkActionKind = 'navigate' | 'start' | 'continue'
export type WorkActionUnavailableReason =
  | 'NO_PROJECT' | 'NO_ENTITY_ANCHOR' | 'CAPABILITY_NOT_CONFIGURED' | 'NO_CANONICAL_TARGET'
  | 'RECORD_NOT_FOUND' | 'RECORD_MISMATCH' | 'NOT_AUTHORIZED' | 'CAPABILITY_UNAVAILABLE' | 'READ_FAILED'

export type WorkActionTarget = { href: string; recordId: string | null }
export type WorkActionIntent = {
  id: string                       // stable: `${nodeId}:${capability}:${kind}`
  capability: WorkActionCapability
  kind: WorkActionKind
  label: string
  target?: WorkActionTarget        // present for navigate
  actionId?: string                // present for start/continue (existing server action)
}
export type ResolvedWorkAction =
  | { status: 'available'; action: WorkActionIntent }
  | { status: 'unavailable'; capability: WorkActionCapability; reason: WorkActionUnavailableReason; message: string }
```

- `deriveWorkType(project, item): WorkType` — pure. If `project.playbookId/version` resolve via
  `getPlaybook` AND `item.id` starts with `${project.id}-` and the suffix matches a node key → that key;
  otherwise `'adhoc'`. Never infer from title/name.
- `WORK_TYPE_CAPABILITIES: Record<WorkType, readonly WorkActionCapability[]>`:
  `parties→['record']`, `property→['record']`, `agreement→['form']`, `signature→['signature']`,
  `media→['cabinet']`, `marketing→['marketing']`, `accounting→['accounting']`, `adhoc→['record']`.
- `planWorkActions(project, item, links): ResolvedWorkAction[]` — pure mapping from the registry + resolved
  `links` to intents/unavailable. Replay-safe (C7).
- Invariants: registry is data; labels are derived from capability + record state, never from a listing name;
  an unknown work type yields `record` only (no throw).

### C2 — Server-side relationship resolution

`app/portal/projects/action-resolution.ts` implements `resolveWorkActions(projects, items, principal, ports)`
where `ports` is an injected interface (real wiring in `page.tsx`, fakes in tests):

```ts
export interface WorkActionReadPort {
  findListingFormInstance(a: WorkActionAnchor): Promise<FormInstanceRef | null>
  findActiveSignatureRequest(formInstanceId: string): Promise<SignatureRef | null>
  listPropertyCabinet(propertyId: string): Promise<{ documents: number; media: number }>
  listPropertyMarketing(propertyId: string): Promise<{ placements: number }>
  listContractAccounting(anchor: WorkActionAnchor): Promise<{ receivables: number; expenses: number }>
  resolveRecordHref(anchor: WorkActionAnchor): string | null   // canonical route for record capability
}
export interface WorkActionAuthorizePort { authorize(req: { domain: string; action: string; kind: 'query'|'command' }): Promise<boolean> }
```

Anchor precedence (per node): node `entity` → project row anchor of the same type
(`project.personId | propertyId | contractId`). This is required because `project.instantiate` sets no
entity anchor (Evidence); resolution must still work from the project row.

Resolution rules per capability:
- `record` — entity anchor → `resolveRecordHref`; no anchor → `NO_ENTITY_ANCHOR`; no route → `NO_CANONICAL_TARGET`.
- `form` (agreement) — `findListingFormInstance(anchor)` where template is `LISTING_AGREEMENT_TEMPLATE_ID`
  (`LISTING-01`). Found → `navigate` `/portal/forms/${id}`, `kind:'navigate'`. None but person+property
  resolvable → `kind:'start'`, `actionId:'form.create'`. No anchor → `NO_ENTITY_ANCHOR`.
- `signature` — from the agreement form instance → its issued document → `getActiveSignatureRequestForDocument`.
  Active → `kind:'continue'`, `navigate` `/portal/forms/${formInstanceId}` (the surface that shows the request).
  Form exists, no active request → `kind:'start'`, `actionId:'form.send-for-signature'`. No form →
  `RECORD_NOT_FOUND`.
- `cabinet` — property anchor → `listPropertyCabinet`. Non-zero → `navigate` to property media/documents.
  Anchor present, zero records → `navigate` with an "empty" note (never invented content). No property →
  `NO_ENTITY_ANCHOR`.
- `marketing` — property anchor → `listPropertyMarketing` → `navigate` `/portal/marketing` (property-scoped).
  No property → `NO_ENTITY_ANCHOR`.
- `accounting` — contract/deal/person anchor → `listContractAccounting` → `navigate` `/portal/accounting`.
  Contract domain absent → `CAPABILITY_UNAVAILABLE`.

Mismatch rule (AC2): a found record whose owning `person_id`/`property_id`/`deal_id` does NOT intersect the
node's effective anchor → `RECORD_MISMATCH` with a message naming what did not match. Absence → `RECORD_NOT_FOUND`.

Invariants:
- Reads are batched by distinct anchor id (no per-node query fan-out — WS-17 guard).
- Every port failure is caught and mapped to `READ_FAILED`; the page still renders (captured via
  `captureServerError`, never a thrown 500, never fabricated data).
- Expected outcomes (no anchor, no record, mismatch, denied) are audited control flow — no `app_error` rows.

### C3 — Authorization enforcement

- Before an action is marked `available`, call `WorkActionAuthorizePort.authorize` with the capability's
  domain/action. Action codes are REUSED from the capability's own route guard, not invented:
  forms/signature/cabinet/record → `deal.read` (matches `app/portal/documents`), projects → `project.read`,
  marketing/accounting → `portal.read`. Lead must confirm each against the route's `resolvePortalAccess` call
  before wiring (Risks 4).
- Denied → `{ status:'unavailable', reason:'NOT_AUTHORIZED', message }`. Never captured as an error row.
- `start`/`continue` require the capability's WRITE guard at execution time (the existing server action
  enforces it); the resolver's read check only governs whether the action is offered.

### C4 — Canonical record navigation

- Navigation is a stable `href` string built from the capability route + the canonical record id. The Lead
  MUST confirm the target route addresses a single record (query param or path); if it cannot, return
  `NO_CANONICAL_TARGET` rather than a dead link.
- No route may be invented; reuse the routes in "Touched surfaces".
- Property uses the existing property workspace/route; person uses the clients workspace; contract/deal uses
  the deals workspace. These workspaces are lens-based (no `[id]` segment); the id is carried as the existing
  selection query parameter, or the capability returns `NO_CANONICAL_TARGET` if none is supported.

### C5 — Workflow start / continue

- `kind` is a pure function of the linked record: absent → `start`; present → `continue`/`navigate`.
- `start`/`continue` DELEGATE to existing actions (`createFormAction`, `sendFormForSignatureAction`) — no new
  capability logic. The agreement spine is: form instance → issue → signature → execution.
- A direct workflow-engine deep link (`/portal/workflows/[instanceId]`) is used ONLY when the record already
  carries a canonical `process_instance`; otherwise it is a follow-up, not fabricated.

### C6 — Unavailable reasons (explicit failure)

Every unavailable action carries a `WorkActionUnavailableReason` and a human `message` that names the cause
(e.g. "No listing agreement form is linked to this project yet", "This record belongs to a different
property"). Pane 3 renders these as text, not as disabled/dead buttons.

### C7 — Replay-safe domain completion projection

- `projectWorkNodeCompletion(node, links): ProjectWorkStatus | null` is pure: same `(node, links)` →
  identical output, no `Date.now()`, no ordering dependence, no side effects, idempotent.
- It DERIVES a linked completion signal from canonical record state (e.g. issued form → `complete`,
  active signature → `in-progress`, cabinet records present → `complete`) but NEVER overwrites the persisted
  WBS status (WS-06 owns status). It is exposed as `ProjectWorkNode.linkedStatus` for display only.
- `planWorkActions` and `projectWorkNodeCompletion` must be deterministic across repeated calls with the same
  inputs; the test asserts deep-equality on a second run.

### C8 — Pane 3 rendering

- `navigate` → `<Link href={target.href}>`; `start`/`continue` → button invoking the delegated action with a
  single in-flight latch; `unavailable` → `role="status"` explanatory text.
- Actions are keyed by `action.id`; the existing entity `relatedItems`/`inspector` behavior is preserved.

## Acceptance (verifiable checks)

| # | Check | Assay command |
|---|-------|---------------|
| AC1 | Every listing node resolves to the correct capability + canonical record target (all 7 nodes) | `pnpm exec tsx --test testv2/projects-workspace-07-actions.test.ts` |
| AC2 | Missing / mismatched / unrelated links return a specific unavailable reason (no dead action) | `pnpm exec tsx --test testv2/projects-workspace-07-actions.test.ts` |
| AC3 | Authorization denied → `NOT_AUTHORIZED`, action withheld, no error row | `pnpm exec tsx --test testv2/projects-workspace-07-actions.test.ts` |
| AC4 | Completion projection + action planning are replay-safe (identical on re-run) | `pnpm exec tsx --test testv2/projects-workspace-07-actions.test.ts` |
| AC5 | Agreement/signature start/continue delegates to existing actions; kind derives from record state | `pnpm exec tsx --test testv2/projects-workspace-07-actions.test.ts` |
| AC6 | Projection still maps nodes/actions and preserves WS-01/WS-06 behavior | `pnpm exec tsx --test testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts testv2/projects-workspace-06-inspector.test.ts` |
| AC7 | Types + diff clean | `pnpm exec tsc --noEmit` · `git diff --check` |
| HG | Linked-record navigation spot check (Forms/signature/Cabinet/Marketing/Accounting) | HUMAN GATE (WS-18) — not machine-verifiable |

## Assay plan (SCOPED — run exactly this)

```sh
pnpm exec tsx --test testv2/projects-workspace-07-actions.test.ts
pnpm exec tsx --test testv2/projects-service-projection.test.ts testv2/projects-workspace-01-readmodel.test.ts testv2/projects-workspace-06-inspector.test.ts
pnpm exec tsc --noEmit
git diff --check
```

Forbidden: `pnpm test`, `pnpm test:persistence`, `pnpm test:engine`, `pnpm test:app`, npm/yarn test,
any multi-file workflow glob. `next build` is not required for this slice (no route/geometry change); if the
Lead changes a route, add `pnpm exec next build --webpack` and record it.

## Risks (what could invalidate this plan)

1. **No persisted work type (HIGH).** Work type is derived from `${project.id}-${node.key}`; a renamed/duplicated
   item or a project id that breaks the prefix match silently degrades to `adhoc`/`record`. Accepted for this
   slice; follow-up story: add `wbs_item.work_type`/`playbook_key`. The test must cover the derive + fallback.
2. **Instantiate sets no entity anchor (HIGH).** `project.instantiate` creates items with `entity=null`, so most
   real playbook nodes have no node anchor. Resolution MUST fall back to project row anchors, and the test must
   prove a project-row-anchored node resolves. If the Lead instead changes `project.instantiate` to set anchors,
   that is a SAME_UNIT addition (it touches ProjectService) and needs its own projection/service tests.
3. **Route addressability (MED).** Portal workspaces are lens-based, not `[id]` routes. A dead link would fail
   the human gate; return `NO_CANONICAL_TARGET` unless the route truly selects one record. Lead must verify each
   href.
4. **Authorization code drift (MED).** Inventing action codes would bypass the enforced resolver. Reuse the exact
   code each route passes to `resolvePortalAccess`; verify by reading the route.
5. **Contract domain absent in PROD (MED).** Agreement/accounting resolution must degrade to
   `CAPABILITY_UNAVAILABLE`, never throw or read `contract` unconditionally (PROD lacks it — MEMORY 2026-09-09).
6. **Signature surface (MED).** There is no standalone in-app signing page; the form editor hosts the request.
   Do not invent a `/portal/sign` route.
7. **Read fan-out (MED).** Resolving per node could be N+1 (WS-17). Batch by distinct anchor id.
8. **No browser tooling (LOW).** Navigation is asserted as href strings only; click-through is the HUMAN GATE.
9. **Pre-existing `GuideItem` tsc noise (LOW).** Known; do not broaden scope to fix it.
10. **Story test file must be authored (LOW).** The assay references a file that does not exist yet; the Builder
    creates it red→green.

## Release obligations

- migrationRequired: false (no DDL; read-only over existing columns/tables).
- derivedRefreshRequired: false.
- deploymentRequired: true (Vercel UI + server resolution path on `/portal/projects`).
- DEV/PROD schema/data: untouched.

## Handoff to Lead

Build C1 (`work-actions.ts`, pure + fully tested with fakes) and C2 (`action-resolution.ts` over injected ports)
first so Assay is runnable before any UI wiring; then C3/C4/C5, then C7, then C8. Extract ALL decision logic out
of the React component. Do not re-implement any capability. Report exact files changed and the SCOPED Assay
result. No commit/push from the architect node.

FORGE_PASS_STOP: COMPLETE
