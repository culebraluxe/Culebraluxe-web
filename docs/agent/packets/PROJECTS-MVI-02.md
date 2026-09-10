# PROJECTS-MVI-02 — Canonical Project and real playbook binding

## Story / Goal

One canonical Project service, versioned playbook identity, and service-created
listing plans with proven Person/Property/Contract relationships.

## Canonical truth and ownership

ProjectService owns Project commands and persistence in `project`. WbsService
owns work items only. Catch-Up and Projects consume the same Project contract.
Do not drop `wbs_project` or migrate its historical rows implicitly.

## Verified starting evidence

- Local MVI-01: cefdfc5e95c6d7ee0f70bad9111c41d32bc951de, not pushed.
- GitHub main: b8df15aec90b460ba5f15c24d6343df85f8e10d1; one newer,
  non-overlapping Forge commit relative to the local base.
- DEV: br-solitary-star-axgusezm; PROD: br-snowy-fog-axg3jae2;
  project snowy-salad-48970537, database neondb.
- Both schemas have project, wbs_item, and legacy wbs_project. Neither has
  playbook identity columns/tables.
- Catch-Up is a live caller of WbsService project.list and must be migrated.
- DEV contains one canonical Contract: OFFER-01 / offer_letter / draft.
  There is no canonical Listing Agreement to validate the seed association.
- The Casa Luar seed binds Ana Rivera via person_property.relation_type=interest,
  not a seller/owner relationship. Its Listing Agreement node binds the sole
  Offer Letter, whose subject property is Sunset Point. Do not promote either
  association into listing truth.

## Scope

1. Retire Project operations/types/repository methods from WBS.
2. Move Catch-Up to ProjectService and use canonical Project DTOs.
3. Test ownership, rejection of retired ingress, and existing Project/WBS reads.
4. Subsequent binding portion: versioned playbook identity, transactional
   service-command instantiation, and validated listing anchors. The accepted
   onboarding sequence permits a project before its agreement is drafted;
   Contract binding is optional until a matching Listing Agreement exists.
   A buyer interest must not be promoted into seller evidence. Confirm the
   actual Casa Luar seller before changing the seeded associations.

## DO NOT

- No push, merge, rebase, deployment, full regression, or unrelated UI changes.
- No deletion of legacy data, reset of seed rows, or arbitrary entity selection.
- No business-data writes until real listing binding is established.
- No new schema-dependent implementation without the DEV/PROD verification path.

## Acceptance criteria

- Only ProjectService advertises project.* operations.
- WBS rejects retired project.* ingress without repository writes/events.
- Catch-Up reads canonical Projects and preserves its WBS daily queue/calendar.
- No executable WBS adapter references wbs_project.
- Completing the entire story additionally requires real playbook identity,
  service-created plans, replay/rollback tests, and verified real bindings.

## Targeted verification

```sh
node --import tsx --test testv2/project-service.test.ts testv2/wbs-service.test.ts testv2/projects-service-projection.test.ts testv2/wbs-project-items-repository.test.ts testv2/project-ownership.test.ts testv2/composition.test.ts
node node_modules/typescript/bin/tsc --noEmit
git diff --check
```

## Handoff / commit expectations

Commit the independently verified ownership repair locally. Report this as
partial MVI-02, not a delivered playbook implementation. Preserve MVI-01.

## Stop condition

Pause the binding portion if the real listing relationship is unresolved.
Never substitute mock state to obtain an acceptance pass.

## Ownership repair result

- Removed WBS-owned Project types, operations, and SQL repository methods.
- Migrated the only live caller, Catch-Up, to canonical ProjectService.
- Preserved calendar/layout; added explicit failure display and the existing
  durable service-error sink rather than silently displaying empty results.
- Updated the composition fake for listProjectItems (the original TypeScript
  run excludes testv2; its passing result did not cover this missing member).
- All 25 scoped tests pass. Application TypeScript and diff checks pass.
- No schema, business data, dependency, push, merge, or deployment changes.
- MVI-02 is PARTIAL: no playbook schema/instantiation implementation yet. The
  verified ownership slice must not be called completion of the whole story.
