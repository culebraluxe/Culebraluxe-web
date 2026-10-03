-- Repair the stale `rust/` paths that the 2026-10-03 layout move left in the PLANNED story rows.
--
-- WHY. `80cfc9da` ("refactor(layout): the tree is three tiers - web, middle, db") moved the tree up one level and
-- deleted `rust/`. Every story row authored before that move still points at the old tree, and the QA gate executes
-- `assay_commands` VERBATIM (`forge/src/engine/qa_adjudicate.rs` refuses even a substituted command, as
-- `ASSAY_COMMAND_SUBSTITUTED`), so a stale path is a guaranteed `CMD_FAIL` before the candidate's code is reached.
--
-- Measured on PROD before this load (Planned rows carrying `rust/`):
--   assay_commands 697 | scope 698 | acceptance_criteria 691 | postconditions 688 | architect_brief 9 | goal 3
--   context_refs 1 | preconditions 0 | negative_control_command 0
-- In assay_commands the stale shape is uniform: 697 of 697 are `--manifest-path rust/Cargo.toml`, nothing else.
--
-- THE MAPPING IS EVIDENCE, NOT GUESSWORK. `rust/` -> `` (drop the prefix) is only correct where the file it names
-- exists at the new path. Each rule below was confirmed against the tree on 2026-10-03:
--   * `rust/test-harness/` -> `tests/`   : 651 postconditions rows; `tests/tests/accounting_core__003__expense_categorization.rs`
--                                          and `...__007__p_l_aggregation.rs` both EXIST (test-harness is the package
--                                          name of the `tests/` crate, so `-p test-harness` is unchanged and correct);
--   * `rust/server/` -> `web/`           : `web/src/api/public_ui.rs` EXISTS;
--   * `rust/` -> ``                      : `forge/src/engine/executor.rs`, `db/src/pool.rs` EXIST; covers
--                                          `rust/Cargo.toml` -> `Cargo.toml` (the workspace root manifest, so a bare
--                                          `cargo -p <crate>` resolves from the repo root) and `rust/cli/`, `rust/forge/`,
--                                          `rust/db/`, `rust/middle/`.
-- Order matters: the longer prefixes are replaced first.
--
-- WHAT IS DELIBERATELY NOT TOUCHED.
--   * `rust/core/` — 3 rows name `rust/core/db/src/whatsapp.rs` and `rust/core/domain` (no under-file path). Old
--     `core/db` -> today `db/`, old `core/domain` -> today `middle/`, but the evidence is two path fragments, not a
--     file that exists, so these 3 rows are HELD for per-row treatment rather than guessed at.
--   * `Complete` rows. Their text is history; this load repairs the rows still waiting to run and nothing else.
--   * `status`, `test_mode`, `work_type`, `acceptance_assertions`. This is a TEXT repair. It arms nothing: no row's
--     status changes, and only `Ready` arms a run, so the poller has nothing new to pick up.
--
-- Applied: cli db-tool apply db/loads/fix_planned_rows_stale_rust_paths_2026_10_03.sql prod
-- Idempotent: the guard waits for a `rust/` the update itself removes.

update storyboard_story
set assay_commands      = replace(replace(assay_commands, 'rust/test-harness/', 'tests/'), 'rust/', ''),
    acceptance_criteria = replace(replace(acceptance_criteria, 'rust/test-harness/', 'tests/'), 'rust/', ''),
    scope               = replace(replace(scope, 'rust/test-harness/', 'tests/'), 'rust/', ''),
    postconditions      = replace(replace(postconditions, 'rust/test-harness/', 'tests/'), 'rust/', ''),
    architect_brief     = replace(replace(architect_brief, 'rust/test-harness/', 'tests/'), 'rust/', ''),
    goal                = replace(replace(goal, 'rust/test-harness/', 'tests/'), 'rust/', ''),
    context_refs        = replace(replace(context_refs, 'rust/test-harness/', 'tests/'), 'rust/', ''),
    updated_at          = now()
where status = 'Planned'
  and (assay_commands like '%rust/%'
       or acceptance_criteria like '%rust/%'
       or scope like '%rust/%'
       or postconditions like '%rust/%'
       or architect_brief like '%rust/%'
       or goal like '%rust/%'
       or context_refs like '%rust/%')
  and coalesce(scope, '') not like '%rust/core/%'
  and coalesce(assay_commands, '') not like '%rust/core/%';

-- Receipt: what is left, and the rows held back on purpose.
select count(*) filter (where assay_commands like '%rust/%')     as stale_assay,
       count(*) filter (where scope like '%rust/%')              as stale_scope,
       count(*) filter (where acceptance_criteria like '%rust/%') as stale_crit,
       count(*) filter (where status = 'Planned')                as planned,
       count(*) filter (where status = 'Ready')                  as ready
from storyboard_story;
