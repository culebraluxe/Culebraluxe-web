-- Fix ENG-FORGE-C1-BUILD-INFO-01's stale `rust/` paths, then re-arm it.
--
-- THE BUG THIS DRAINS. `80cfc9da` ("refactor(layout): the tree is three tiers - web, middle, db", 2026-10-03)
-- deleted `rust/`: the workspace root manifest, the CLI, the server and the tests all moved up one level. This
-- story row was authored BEFORE that move and still carries the old paths in the fields the engine reads:
--
--   * `assay_commands` — all four commands are prefixed `--manifest-path rust/Cargo.toml`, and the gate executes
--     that text VERBATIM (`forge/src/engine/qa_adjudicate.rs:102-107` refuses even a substituted command, as
--     `ASSAY_COMMAND_SUBSTITUTED {command}`), so every command dies before it can touch the candidate's code:
--         $ cargo run --manifest-path rust/Cargo.toml -p cli -- forge build-info
--         error: manifest path `rust/Cargo.toml` does not exist
--         cargo exit=101
--   * `acceptance_criteria` — criteria 1, 3, 7 and 8 name that same dead path, so the spec Smith builds to and
--     QA maps against is not runnable as written;
--   * `scope` and `context_refs` — `rust/cli/src/forge/mod.rs` and `rust/cli/src/forge/build_info.rs` are files
--     that no longer exist.
--
-- That is the whole of the 2026-10-03 mis-verdict. QA recorded `CMD_FAIL <command>` for all four commands
-- (`qa_adjudicate.rs:113`), ruled FAIL three times (19:31:13Z, 19:39:47Z, 19:43:26Z), and NO PASS means nothing
-- was published — so the candidate chain (`11a0293b` + two test commits, based on current main) has sat
-- unpublished ever since. QA is not at fault and the code is not at fault: the candidate's own
-- `cli/src/forge/build_info.rs` is correct and the CLI built from it answers `forge build-info` ->
-- `cli_version 0.1.0`, exit 0. The row's TEXT was.
--
-- The blast radius is 698 of 889 rows (`assay_commands like '%manifest-path rust/%'` = 698; the corrected form =
-- 0), because the layout move re-pointed the tree under every story authored before it. This load fixes ONE row:
-- the wiring test the Captain is walking end to end. The other 697 are the bulk question and are deliberately
-- untouched.
--
-- WHAT. Normalise the paths the way the layout move did — drop the dead `--manifest-path rust/Cargo.toml ` flag
-- (the workspace root manifest IS `Cargo.toml`, so bare `cargo -p cli` resolves from the repo root) and
-- `rust/cli/` -> `cli/` — then flip the board `Hold` -> `Ready`, the same action the Captain makes by hand. The
-- Ready trigger inserts at most one item per story and never raises
-- (`db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql`), so the status change is the whole arm.
-- Idempotent: the guard waits for a `rust/` the update itself removes.
--
-- Expected: `forge: rerun of ENG-FORGE-C1-BUILD-INFO-01 retires instance ...`, a fresh run, and a QA gate whose
-- four commands actually RUN — one story all the way through.
--
-- Applied: cli db-tool apply db/loads/fix_eng_forge_c1_stale_rust_paths_2026_10_03.sql prod

update storyboard_story
set assay_commands        = replace(replace(assay_commands, '--manifest-path rust/Cargo.toml ', ''), 'rust/cli/', 'cli/'),
    acceptance_criteria   = replace(replace(acceptance_criteria, '--manifest-path rust/Cargo.toml ', ''), 'rust/cli/', 'cli/'),
    scope                 = replace(replace(scope, '--manifest-path rust/Cargo.toml ', ''), 'rust/cli/', 'cli/'),
    context_refs          = replace(replace(context_refs, '--manifest-path rust/Cargo.toml ', ''), 'rust/cli/', 'cli/'),
    updated_at            = now()
where id = 'ENG-FORGE-C1-BUILD-INFO-01'
  and (assay_commands like '%rust/%'
       or acceptance_criteria like '%rust/%'
       or scope like '%rust/%'
       or context_refs like '%rust/%');

update storyboard_story
set status = 'Ready',
    updated_at = now()
where id = 'ENG-FORGE-C1-BUILD-INFO-01'
  and status = 'Hold';

select s.id,
       s.status                                                          as board,
       s.assay_commands                                                  as commands,
       left(s.context_refs, 64)                                          as refs,
       (select count(*) from forge_hold_record h where h.story_id = s.id) as holds,
       (select count(*) from storyboard_story_run r where r.story_id = s.id) as runs,
       (select count(*) from agent_work_item w
         where w.story_id = s.id and w.state in ('Queued','Claimed'))    as open_items
from storyboard_story s
where s.id = 'ENG-FORGE-C1-BUILD-INFO-01';
