-- CulebraLuxe
-- LOAD: declare_tst_fast_lane_2026_09_30.sql
--
-- THE DATA HALF OF WIRING THE FAST LANE (migration 259 is the schema half).
--
-- The TST backlog is bounded, pre-shaped work whose acceptance bar is "it compiles": a Rust test file inside
-- `rust/test-harness`, no migration, no deployment, no architecture decision. Every one of these stories is
-- eligible for the fast lane (`FORGE_SDLC-v6.xml:85`, gate `forge_fast_eligibility`, `facts.rs:247`), and until
-- migration 259 the board had no way to say so — all 680 armed rows were dispatched as FEATURE and paid a Scout,
-- an Architect and a Lead turn each. One of the first four arms died in the Architect turn.
--
-- This is a REVERSIBLE DECLARATION, not a rewrite: it sets one column on the story rows, and copies it onto the
-- queue rows that are already open (the Ready trigger only fires at dispatch, so rows armed before migration 259
-- do not carry the copy).
--
-- `updated_at` ON `agent_work_item` IS DELIBERATELY NOT TOUCHED. It is the claim heartbeat: `stale_agent_work`
-- decides staleness and requeue on it alone, so stamping it from a data load would keep a dead claim looking alive
-- or resurrect a live one's window. `storyboard_story` is left to whatever already maintains it.
--
-- REVERSE: update storyboard_story set work_type=null where id like 'TST-%'; then the same copy, downward.

begin;

-- 1. The declaration, on the board.
update storyboard_story
   set work_type = 'FAST'
 where id like 'TST-%'
   and work_type is distinct from 'FAST';

-- 2. The copy, onto every queue row that is already open, so the backlog that is armed right now enters the lane
--    without being re-dispatched. Closed rows are left alone: they are history.
update agent_work_item w
   set work_type = s.work_type
  from storyboard_story s
 where s.id = w.story_id
   and s.work_type is not null
   and w.state in ('Ready', 'Claimed', 'Running', 'Paused')
   and w.work_type is distinct from s.work_type;

-- 3. What landed. Declared stories, then the open queue rows that now carry the fast lane.
select 'storyboard_story(FAST)' as fact, count(*) as n
  from storyboard_story where work_type = 'FAST'
union all
select 'agent_work_item(FAST,open)', count(*)
  from agent_work_item
 where work_type = 'FAST' and state in ('Ready', 'Claimed', 'Running', 'Paused');

commit;
