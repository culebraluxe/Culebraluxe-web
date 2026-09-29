-- Promote ONE row: the harness foundation story, and nothing else.
--
-- WHY ONLY ONE. 689 TST-* rows were loaded into PROD on 2026-09-29 (688 test stories + this foundation row).
-- All 688 test rows fence on the same first token:
--
--   cargo test --manifest-path rust/Cargo.toml -p test-harness --test <suite>__<NNN>__<name>
--
-- and `test-harness` is not a workspace member (rust/Cargo.toml members are core/{domain,db,workflow,auth,service},
-- forge, server, integrations, cli, ui) and exists nowhere on disk. So all 688 fences fail on that token
-- today with "package ID specification `test-harness` did not match any packages".
--
-- The one row that does not is TST-HARNESS-FOUNDATION-001: its assay is `cargo test -p test-harness` with no
-- --test filter, i.e. an empty crate, and its acceptance criteria are the story that creates the crate
-- (deterministic clock/ID helpers, database isolation that refuses PROD and owns its cleanup, concurrency
-- barriers, Axum request helpers, deterministic fault injection, fake external providers, MVI and snapshot
-- helpers, with the one-way rule that production crates never depend on the harness).
--
-- WHY THIS ONE FIRST. 688 of 689 rows carry dependencies = 'TST-HARNESS-FOUNDATION-001'. That dependency is
-- documentation only: nothing in rust/forge reads the dependencies column, because the agent packet is
-- id + title + goal + architect_brief + acceptance_criteria + assay_commands
-- (rust/core/db/src/forge_engine.rs:1574, rust/forge/src/engine/packet.rs:19-60). The ordering therefore
-- lives in this file and in the promotions that follow it. Do not promote a test row before this one is Done.
--
-- WHY IT IS SAFE NOW. The claim path orders by priority: `order by w.priority desc, w.queued_at asc, w.id`
-- (rust/core/db/src/forge_engine.rs:752), and this row is Critical (story_priority_score = 100), so it is
-- taken before the seven High (80) items already queued on PROD. The Ready trigger inserts exactly one item
-- per Ready story and never raises: `on conflict (story_id) ... do nothing`
-- (db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql:39-49).
--
-- Idempotent on id AND on status: the `status = 'Planned'` guard makes a re-run a no-op, so applying this
-- file twice can never re-dispatch a row that has already been picked up.
--
--   ./rust/target/debug/cli db-tool apply db/loads/promote_harness_foundation_2026_09_29.sql prod \
--     --note "TST-HARNESS-FOUNDATION-001: the only row whose fence is legitimate today"

begin;

update storyboard_story
   set status = 'Ready',
       updated_at = now()
 where id = 'TST-HARNESS-FOUNDATION-001'
   and status = 'Planned';

commit;
