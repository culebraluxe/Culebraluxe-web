# CulebraLuxe Server Command → Authority Map

Status: AUTH-02 route/portal enforcement is ACTIVE (middleware cheap gate +
authoritative server-side layout guards). Per-action server-action enforcement
is AUTH-03 ACTIVE — every write action in `app/portal/actions.ts` runs through
`portalWrite(authority, handler)` (runAuthorized on the portal session adapter)
and both media upload routes (`app/api/media/upload`,
`app/api/property-media/upload`) guard with `listing.write` before any write
work.

Coarse authorities only. No per-button authorities. Business-state legality stays with domain/workflow services.

## Writes (`app/portal/actions.ts`)

| Server action | Authority |
|---------------|-----------|
| `createTaskAction`, `completeTaskAction`, `cancelTaskAction`, `updateTaskDueAction` | `crm.write` |
| `logManualInteractionAction` | `crm.write` |
| `updatePersonNotesAction`, `updatePersonStatusAction` | `crm.write` |
| `resolveIntakeAction` (attach | create | reject) | `crm.write` |
| `createShowingAction`, `scheduleShowingAction`, `cancelShowingAction`, `completeShowingAction` | `deal.write` |
| `submitOfferAction`, `withdrawOfferAction`, `rejectOfferAction` | `deal.write` |
| `addOtherParticipantAction`, `endParticipantAction`, `updateParticipantRoleLabelAction` | `deal.write` |
| `updatePropertyFactsAction`, `updatePropertyVisibilityAction` | `listing.write` |
| `setPropertyMediaOrderAction`, `setPropertyHeroAction`, `unlinkPropertyMediaAction`, `updateMediaMetadataAction` | `listing.write` |
| Future settings mutations (assign roles, manage roles/authorities) | `settings.manage` |

## Reads

| Surface | Authority |
|---------|-----------|
| Portal dashboards / operational views | `portal.read` |
| Deal workspaces / deal listing | `deal.read` |
| Settings views (users/roles/authorities/status) | `settings.read` |

## Boundary notes

- `resolveIntakeAction` remains `crm.write` at the authority layer, but the *business* decision to attach or create a person is identity-sensitive and will additionally be workflow-gated (not expressed as a separate authority).
- `completeShowingAction`, `submitOfferAction`/`rejectOfferAction`/`withdrawOfferAction`, and `endParticipantAction` are consequential commands; they are `deal.write` at the authority layer and will carry CRM-14 business-state gating separately.

## Enforcement layers (AUTH-02)

- **Route/resource (coarse, Edge)**: `middleware.ts` applies `PORTAL_ROUTE_POLICY`
  (`lib/auth/route-policy.ts`): unauthenticated `/portal*` → `/login`;
  authenticated-but-missing the required authority (from the JWT capability
  snapshot) → `/login/unauthorized`; `/portal/settings*` additionally requires
  `settings.read`. Cheap first gate only — the Edge runtime does not resolve
  authorities from the DB.
- **Route/resource (authoritative, server-side)**: the Portal layout
  (`app/portal/layout.tsx`) calls `getActingUser` + `requireAuthority('portal.read')`
  through `resolvePortalAccess` and redirects on AuthError; the settings layout
  (`app/portal/settings/layout.tsx`) re-checks `settings.read` server-side.
- **UI projections**: client components receive a serialized actor snapshot
  (authority codes) from the server to hide nav/buttons. Cosmetic only — never
  the security boundary; a direct call to a protected surface still fails
  server-side.
- **Server actions**: `runAuthorized` (AUTH-03) is the seam; every write action
  in `app/portal/actions.ts` wraps its business service with
  `portalWrite(<authority>, handler)` per the map above. Upload route handlers
  guard via `guardPortalUpload(<authority>)` before multipart/write work.

## Read-scoping rule (deal reads)

Internal coarse reads (`portal.read` / `deal.read`) return all rows. External
clients hold only `external.deal.read_own` (never `portal.read`) and their deal
reads are scoped in the deal read services to deals linked to the actor's own
person (`app_user.person_id`) via active `deal_participant` rows — not a new
authority. An external actor without a linked person gets no rows; an external
actor requesting a deal they do not participate in gets the empty workspace.
Applied at: `db/deals.ts getDeals(actor?)`, `db/deal-workspace.ts
getDealWorkspace(dealId, actor?)`.
