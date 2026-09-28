-- CulebraLuxe
-- Batch-sliced deployment for major feature rollouts.
-- Migration: 148_story_batch_deploy.sql
--
-- WHY: a multi-story feature rollout (e.g. the Projects workspace, 19 stories)
-- must NOT deploy to production story-by-story. DEV_OPS therefore needs a way to
-- complete a story at QA-verified with its DEPLOYMENT DEFERRED to a batch, as a
-- recorded fact — not a fake deployment receipt and not a prose instruction a model
-- may ignore.
--
-- `batch` already groups stories into release slices. This column says whether the
-- slice deploys as one deliberate batch.
--
-- Semantics:
--   batch_deploy = true  -> the deploy stage is DEFERRED. The story may complete as
--                           QA-verified/published with `deployment_deferred_to_batch`
--                           recorded; nothing claims a verified deployment.
--   batch_deploy = false -> unchanged: the deploy stage must produce a real receipt.
--
-- Non-destructive: additive column with a default, no rewrite of existing semantics.

begin;

alter table storyboard_story
    add column if not exists batch_deploy boolean not null default false;

comment on column storyboard_story.batch_deploy is
    'Major rollout slicing: when true, the story completes QA-verified with its deployment DEFERRED to the release batch (never a fabricated deployment receipt). Release via scripts/forge-batch-release.mjs.';

-- Forge evidence: a distinct, honest terminal state for a deferred deployment.
alter table forge_workflow_evidence
    add column if not exists deployment_deferred_to_batch integer;

comment on column forge_workflow_evidence.deployment_deferred_to_batch is
    'Set when a batch-sliced story completes with deployment deferred (see storyboard_story.batch_deploy). Distinct from a real deployment receipt: it never claims production verification.';

commit;
