-- Unblock ENG-FORGE-C1-BUILD-INFO-01: retire the DEAD-LANE vendor session, then flip the board.
--
-- THE BUG THIS DRAINS. `forge/src/engine/opencode.rs:519-533` picks the turn's session by preferring
-- `forge_vendor_session` (keyed story_id + lane) over the workspace marker `read_session_id(&cwd)`. That row is
-- per STORY, but a vendor session belongs to the project DIRECTORY it was created in — and
-- `derive_worktree_path(root, story_id, run_id)` (`forge/src/engine/worktree.rs:82`) puts the RUN id in the path,
-- so a flip mints a new directory and leaves the stored session behind in a directory the engine has deleted.
--
-- On this machine that is not a theory. From the rows, not a log:
--
--   * `forge_vendor_session` holds `ses_f00630540ffeBPyVxLkQ9GKOWW` for (ENG-FORGE-C1-BUILD-INFO-01, opencode-v2),
--     last written 02:42:50 — the 02:34 run's Lead turn (the paid work the 42804 hold stranded);
--   * the directory that session lives in (`.../culebraluxe-forge-worktrees/eng-forge-c1-build-info-01-cd83e384-...`)
--     does not exist: the engine removes its worktree, and the whole `culebraluxe-forge-worktrees` root is gone;
--   * every turn since has re-passed that dead id, and the vendor answers before the first token:
--     `UnexpectedStatus: 500`, tokens_in=0, cost_usd=0.000000 — three attempts, run #5, #6 and #7, $0.00;
--   * reproduced on demand, same binary, same flags: with no `--session` the turn answers `ok`; with
--     `--session ses_f00630540ffeBPyVxLkQ9GKOWW` from any other directory it returns
--     `{"type":"error","sessionID":"","error":{"type":"unknown","message":"UnexpectedStatus: 500"}}`.
--
-- So the 500 is not the model, not the agent (`--agent forge-architect` answers `ok` fresh), not the vendor's
-- health, and not the hold bug (42c73e16 landed the cast four hours before the 02:42:53 failure that recorded it).
-- It is Forge resuming a session across a directory change, which is what the vendor's own 500 says.
--
-- WHAT. Clear the one dead row so the next architect turn mints a fresh session in the new worktree, then flip the
-- board the way the Captain does. `session_id` is nullable and `None` is a state Forge itself writes
-- (`forge/src/engine/vendor_session.rs:65-78`), so this is the supported "no session yet" value, not a hack.
-- The PERMANENT half is the code fix in `forge/src/engine/opencode.rs`: resume the stored session only when it
-- belongs to the turn's own directory.
--
-- Expected: the next tick claims the story, `forge: rerun of ENG-FORGE-C1-BUILD-INFO-01 retires instance
-- 340d5440...`, a fourth instance and a fresh run, and an architect turn that SPENDS tokens instead of 500ing.
--
-- Applied: `cli db-tool apply db/loads/unblock_eng_forge_c1_stale_vendor_session_2026_10_03.sql prod`

update forge_vendor_session
set session_id = null,
    updated_at = now()
where story_id = 'ENG-FORGE-C1-BUILD-INFO-01'
  and worker_id = 'opencode-v2'
  and session_id is not null;

update storyboard_story
set status = 'Ready',
    updated_at = now()
where id = 'ENG-FORGE-C1-BUILD-INFO-01'
  and status = 'Hold';

select s.id,
       s.status                                     as board,
       (select count(*) from forge_hold_record h
         where h.story_id = s.id)                   as holds,
       (select count(*) from storyboard_story_run r
         where r.story_id = s.id)                   as runs,
       (select count(*) from forge_vendor_session v
         where v.story_id = s.id
           and v.session_id is not null)            as live_sessions
from storyboard_story s
where s.id = 'ENG-FORGE-C1-BUILD-INFO-01';
