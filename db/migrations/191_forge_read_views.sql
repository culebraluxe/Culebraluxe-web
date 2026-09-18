-- CulebraLuxe
-- ENG-FORGE-READ-TOOLS-01 — the daily reads become sanctioned, read-only shapes.
-- Migration: 191_forge_read_views.sql
--
-- The operator keeps hand-writing the same SQL per investigation: board counts, a story's
-- full picture, the latest run receipt, why a story is held, and where its change set sits
-- in the migration ledger. Each hand-written read is a second interpretation of a fact.
--
-- These five VIEWS are the sanctioned read shapes. A view is read-only by construction:
-- there is no INSTEAD OF trigger and no rule, so it can never become a second writer. The
-- CLI (scripts/forge-read-tools.ts) selects ONLY from these views.
--
-- NORMALIZATION: ids and timestamps leave the database in the driver's native form and the
-- CLI normalizes them (ISO string or null) at the boundary, matching the existing readers.
--
-- Non-destructive: creates/replaces views only. No table, row or column is touched.

begin;

-- SET BEFORE THE DDL, and the reason is not ceremony: a migration that waits on a lock queues every
-- query behind it, so it can take production down without ever failing. Squawk refused this file on
-- both rules the moment the CI gate went live (2026-09-18, run 35339022470) — the engine's own static
-- gate does not run squawk yet, which is exactly what ENG-FORGE-MIGRATION-LINT-01 exists to fix.
-- Nothing has applied this migration (schema_migration has no 191), so this is a fix, not a rewrite.
set lock_timeout = '5s';
set statement_timeout = '30s';


-- 1. BOARD BY BATCH — one row per (batch, story) with the batch's own counts.
--    Serves `forge:board` (grouped by batch) and the board state of `forge:story:show <id>`.
create or replace view forge_board_by_batch as
select
  b.id                                as batch_id,
  b.label                             as batch_label,
  b.status                            as batch_status,
  b.scheduled_for                     as batch_scheduled_for,
  b.fired_at                          as batch_fired_at,
  b.created_at                        as batch_created_at,
  b.created_by                        as batch_created_by,
  b.note                              as batch_note,
  b.model_policy                      as batch_model_policy,
  i.story_id                          as story_id,
  i.state                             as item_state,
  i.queued_at                         as item_queued_at,
  i.error_text                        as item_error_text,
  s.title                             as story_title,
  s.status                            as story_status,
  count(*) over (partition by b.id)   as story_count,
  count(*) filter (where i.state = 'Queued')  over (partition by b.id) as queued_count,
  count(*) filter (where i.state = 'Skipped') over (partition by b.id) as skipped_count
from forge_batch b
join forge_batch_item i on i.batch_id = b.id
left join storyboard_story s on s.id = i.story_id;

comment on view forge_board_by_batch is
  'Sanctioned board read: one row per (batch, story) with the batch counts. The CLI groups by batch_id and maps through the existing mapForgeBatch normalizer.';

-- 2. STORY RUN RECEIPT — the latest run for each story: its verdict and its commit.
create or replace view forge_story_run_receipt as
select
  r.id             as run_id,
  r.story_id       as story_id,
  r.result_status  as result_status,
  r.commit_hash    as commit_hash,
  r.tests_summary  as tests_summary,
  r.completion     as completion,
  r.started_at     as started_at,
  r.ended_at       as ended_at,
  r.created_at     as created_at
from storyboard_story_run r
where r.id = (
  select r2.id
  from storyboard_story_run r2
  where r2.story_id = r.story_id
  order by r2.created_at desc, r2.id desc
  limit 1
);

comment on view forge_story_run_receipt is
  'Sanctioned receipt read: the newest storyboard_story_run per story (verdict = result_status, commit = commit_hash).';

-- 3. OPEN HOLDS — every unresolved hold with the reason it parked.
create or replace view forge_open_holds as
select
  h.id                  as hold_id,
  h.story_id            as story_id,
  h.process_instance_id as process_instance_id,
  h.task_id             as task_id,
  h.reason              as reason,
  h.originating_node    as originating_node,
  h.failure_class       as failure_class,
  h.resume_target       as resume_target,
  h.created_at          as created_at
from forge_hold_record h
where h.resolved_at is null;

comment on view forge_open_holds is
  'Sanctioned hold read: unresolved forge_hold_record rows. A blank reason is normalized by the CLI to "unknown", never "".';

-- 4. STORY FINDINGS / CONTRACT — the architect findings recorded for a story.
create or replace view forge_story_findings as
select
  f.id                  as finding_row_id,
  f.finding_id          as finding_id,
  f.story_id            as story_id,
  f.process_instance_id as process_instance_id,
  f.task_id             as task_id,
  f.node_id             as node_id,
  f.attempt             as attempt,
  f.summary             as summary,
  f.required            as required,
  f.seams               as seams,
  f.hint                as hint,
  f.preconditions       as preconditions,
  f.postconditions      as postconditions,
  f.classes             as classes,
  f.risks               as risks,
  f.created_at          as created_at
from forge_role_finding f;

comment on view forge_story_findings is
  'Sanctioned findings read: forge_role_finding rows, one per task/node/attempt/finding_id.';

-- 5. MIGRATION LEDGER STATE — the story's declared change set joined to the ledger.
create or replace view forge_migration_ledger as
select
  e.story_id              as story_id,
  e.process_instance_id   as process_instance_id,
  e.migration_required    as migration_required,
  e.dev_migration_applied as dev_migration_applied,
  e.dev_migration_verified as dev_migration_verified,
  e.prod_migration_applied as prod_migration_applied,
  e.prod_migration_verified as prod_migration_verified,
  f.filename              as filename,
  m.id                    as migration_id,
  m.checksum              as checksum,
  m.target                as target,
  m.note                  as note,
  m.applied_at            as applied_at
from forge_workflow_evidence e
cross join lateral jsonb_array_elements_text(coalesce(e.migration_files, '[]'::jsonb)) as f(filename)
left join schema_migration m on m.filename = f.filename;

comment on view forge_migration_ledger is
  'Sanctioned ledger read: each migration file the story declared in forge_workflow_evidence.migration_files, joined to schema_migration. A declared file with no ledger row keeps migration_id null rather than disappearing.';

commit;
