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

alter table storyboard_story_run
  add constraint storyboard_story_run_commands_total_nonnegative
    check (commands_total is null or commands_total >= 0),
  add constraint storyboard_story_run_commands_passed_nonnegative
    check (commands_passed is null or commands_passed >= 0),
  add constraint storyboard_story_run_commands_failed_nonnegative
    check (commands_failed is null or commands_failed >= 0),
  add constraint storyboard_story_run_tests_total_nonnegative
    check (tests_total is null or tests_total >= 0),
  add constraint storyboard_story_run_tests_passed_nonnegative
    check (tests_passed is null or tests_passed >= 0),
  add constraint storyboard_story_run_tests_failed_nonnegative
    check (tests_failed is null or tests_failed >= 0),
  add constraint storyboard_story_run_policy_violation_count_nonnegative
    check (policy_violation_count is null or policy_violation_count >= 0);
