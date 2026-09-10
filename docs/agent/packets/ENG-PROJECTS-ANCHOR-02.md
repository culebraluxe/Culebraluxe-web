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

## Run log

### 2026-09-10 — engine run (Astra routing refinement proof)

Chain: `architect → lead_pre → smith → lead_post → qa_verify → deploy`.

- `lead_pre`: **`lead-routing verdict=GO errors=none`**, decision `SMITH`, with the standing
  advisory `self-rated NOT_DISPATCHABLE: out-of-range risk: worker-fit (worker would be set
  up to fail) — advisory only, not a dispatch veto`. The identical self-rating HOLDed the
  previous night; it is now recorded dissent, not a veto.
- `smith`: candidate `0465126bd8bfe55306d98cfd7272cfc53b9188a8`; `lead_post` integrated the
  same SHA; `qa_verify` reported `qaPassed: true`.
- `deploy`: **HOLD (after 2 attempts): role did not deliver `devops-receipt`** — a
  pre-existing structural gap, not a routing failure. Nothing in the repo ever populates
  `AgentRunEvidence.releaseEvidence`, which the `deploy` adjudication requires, so the stage
  cannot be satisfied by any run (see MEMORY 2026-09-10 "ENGINE BLOCKER"). The story is
  therefore NOT complete; the routing refinement it was used to prove is.
- Story status after the run: `In Progress` (candidate QA-verified, release stage blocked).
