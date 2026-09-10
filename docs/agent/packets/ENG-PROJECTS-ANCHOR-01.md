# ENG-PROJECTS-ANCHOR-01 — Projects workspace: Documents/Activity fall back to WBS entity anchors

## Goal

The Projects workspace Pane 2 has Documents and Activity views that render **empty**
for every project. The projection matches documents and activity on the **project
row's** `propertyId` / `personId`, but those columns are `null` (or synthetic) on the
real projects — their anchors live on the project's **WBS items** as `entity_type` /
`entity_id` (e.g. `property` → a real property uuid, `person` → a real person uuid).

Derive **effective anchors** for a project: prefer the project row's own
`personId` / `propertyId`; when the row has none, fall back to the unique
`person` / `property` entity anchors among that project's WBS items. Use the
effective anchors when attaching documents and activity.

## Scope

- `ui/projects/service-projection.ts` — compute effective anchors per project and use
  them for the `documents` and `activity` projections.
- `testv2/projects-service-projection.test.ts` — add the cases below.

Do NOT change any component, service, repository, or schema. Do NOT special-case any
listing. Do NOT invent anchors: a project with no row anchor and no entity anchor
must still project empty documents/activity.

## Acceptance criteria

- [ ] A project whose row has no `propertyId` but whose WBS items carry a `property`
      entity anchor attaches **that property's** documents.
- [ ] A project whose row has no `personId` but whose WBS items carry a `person`
      entity anchor matches **that person's** activity entries.
- [ ] A project with neither a row anchor nor an entity anchor projects **empty**
      documents and activity (no accidental match-everything).
- [ ] A project row anchor still wins when present (row value is not overridden by a
      WBS anchor).
- [ ] Existing projection tests still pass unchanged.

## Test mode

SCOPED

## Assay commands

- `pnpm exec tsx --test testv2/projects-service-projection.test.ts`
- `pnpm exec tsc --noEmit`
