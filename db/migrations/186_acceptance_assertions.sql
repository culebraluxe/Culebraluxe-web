-- 186_acceptance_assertions.sql
--
-- ENG-FORGE-ACCEPTANCE-SUPPLIER-01: the acceptance-to-assertion mapping gets a producer.
--
-- The reader already existed (`collectAssayEvidence`), and so did the builder
-- (`buildStoryAcceptanceMap`), but nothing ever SUPPLIED the mapping on the QA port, so every story
-- in the factory came back UNPROVEN with `acceptance-map-missing`. This migration gives the mapping
-- the two durable homes the story names, so a declaration has somewhere to live:
--
--   * `storyboard_story.acceptance_assertions` — the story author's declaration, beside the
--     acceptance criteria it maps.
--   * `forge_role_contract.acceptance_assertions` — the handoff declaration, written by the
--     Architect/Lead through `scripts/forge-handoff.mjs --acceptance-assertion`.
--
-- ONE READER resolves them in a stated order (handoff first, story fallback, source named) in
-- `workflow_app/forge/forge-architect-contract.ts`. Both columns are NULLABLE and additive: an
-- absent mapping stays absent (never `{}`), so the existing UNPROVEN `acceptance-map-missing`
-- blocker is unchanged for a story that declares nothing.
--
-- Non-destructive: two added nullable columns, no existing row or column touched.

alter table storyboard_story
    add column if not exists acceptance_assertions jsonb;

alter table forge_role_contract
    add column if not exists acceptance_assertions jsonb;

comment on column storyboard_story.acceptance_assertions is
    'Clause -> assertion refs declared by the story author (ENG-FORGE-ACCEPTANCE-SUPPLIER-01). Null = not declared; never {} .';

comment on column forge_role_contract.acceptance_assertions is
    'Clause -> assertion refs declared through the handoff (ENG-FORGE-ACCEPTANCE-SUPPLIER-01). Null = not declared; never {} .';
