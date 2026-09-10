# ENG-PROJECTS-ANCHOR-02 — Projects projection: report the effective anchor source

## Goal

`mapRealProjectsToWorkspace` now derives each project's **effective** documents/activity
anchors (the project row's value when present, else the distinct person/property entity
anchors from that project's WBS items). When a project has no usable anchor, Pane 2's
Documents and Activity views render **empty**, which reads as broken rather than as
"nothing is linked yet".

Surface the provenance: `ProjectPlan` gains an optional `anchorSource` describing where
the documents/activity anchors came from — `'row'` (project row anchors), `'wbs'`
(derived from the project's WBS entity anchors) or `'none'` (neither), so the workspace
can explain an empty pane instead of appearing to fail.

## Scope

- `ui/projects/model.ts` — add the optional `anchorSource?: 'row' | 'wbs' | 'none'` field
  to `ProjectPlan`.
- `ui/projects/service-projection.ts` — set it per project while deriving the anchors.
- `testv2/projects-service-projection.test.ts` — add the cases below.

Do NOT change any component, service, repository or schema. Do NOT change how documents
or activity are filtered — this is provenance only.

## Acceptance criteria

- [ ] A project whose row carries a person/property anchor reports `anchorSource: 'row'`.
- [ ] A project whose row has no anchor but whose WBS items carry a person/property
      entity anchor reports `anchorSource: 'wbs'`.
- [ ] A project with neither reports `anchorSource: 'none'`.
- [ ] Existing projection tests still pass unchanged.

## Test mode

SCOPED

## Assay commands

- `pnpm exec tsx --test testv2/projects-service-projection.test.ts`
- `pnpm exec tsc --noEmit`
