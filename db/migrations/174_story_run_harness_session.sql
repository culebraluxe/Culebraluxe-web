-- CulebraLuxe Portal
-- FORGE run rows remember WHICH harness session they executed in.
-- Migration: 174_story_run_harness_session.sql
--
-- ENG-FORGE-WARM-SESSION-01: one live OpenCode session per execution generation. The
-- session id is the identity of that generation's desk, and until now it was nowhere in
-- the control plane — the adapter created a session per role, threw it away, and the next
-- role paid to re-read the repository into a brand new one.
--
-- Storing it does three jobs:
--   1. it is what the NEXT role pins with `--session <id>` (exact, not "the project's
--      last session");
--   2. it attributes spend to a session, which is the COST-01 precondition (tokens and
--      cost already ride the same run row since the accounting capture landed);
--   3. it makes "did this generation reuse one desk or pay for five?" answerable from
--      data instead of from a log.
--
-- Nullable on purpose: older runs, model-free lanes (the deterministic Assay) and an
-- unreadable harness store all mean unmeasured, never a fabricated id.

begin;

alter table storyboard_story_run
    add column if not exists harness_session_id text null;

comment on column storyboard_story_run.harness_session_id is
    'OpenCode session id this run executed in. Same value across the roles of one execution generation; new value after a REPLAN. Null = unmeasured (older runs, model-free lanes, unreadable harness store).';

commit;
