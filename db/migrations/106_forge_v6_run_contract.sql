-- Forge V6 — keep the existing Story -> Story Run architecture.
-- No child event table. Each run stores the frozen architect contract plus
-- generic machine facts that apply across Smith, Assay, Scout, Inspector and
-- future lanes. Historical rows are intentionally not backfilled from today's
-- Story because that would fabricate old execution context.

alter table storyboard_story_run
  add column if not exists scope_snapshot text,
  add column if not exists dependencies_snapshot text,
  add column if not exists operating_surface_snapshot text,
  add column if not exists test_mode_snapshot text,
  add column if not exists assay_commands_snapshot text,
  add column if not exists packet_sha_snapshot text,
  add column if not exists base_commit_hash text,
  add column if not exists commands_total integer,
  add column if not exists commands_passed integer,
  add column if not exists commands_failed integer,
  add column if not exists tests_total integer,
  add column if not exists tests_passed integer,
  add column if not exists tests_failed integer,
  add column if not exists policy_violation_count integer,
  add column if not exists failure_code text,
  add column if not exists evidence_detail text;
