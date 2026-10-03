-- CulebraLuxe
-- Migration: 259_forge_declared_work_type.sql
--
-- WIRE THE FAST LANE (captain's word, 2026-09-30).
--
-- THE GAP. The engine has had a FAST lane since `FORGE_SDLC-v6.xml:85`
-- (`workType == 'FAST'` → `fast_lane_entry` → `fast_smith` → `fast_qa_verify`: no Scout, no Architect,
-- no Lead, no DEV_OPS model runs, deterministic QA never skipped) and migration 173 made `FAST` a
-- work type the ledger accepts. Nothing could ENTER it. The entry fact is the work type, and on the
-- queue path the work type is derived from `agent_work_item.kind`
-- (`forge/src/engine/worker.rs:126`), whose vocabulary is the BATCH's six words — `qa`, `fix`,
-- `feature`, `crm`, `judgment`, `learn` (migration 179) — which map to BUG / RESEARCH / FEATURE and
-- cannot express FAST. And `kind` is NULL on every item the board queues: the Ready trigger inserts
-- `(story_id, state, priority)` and nothing else (`146:40`). So every story the board dispatches ran
-- as a FEATURE story. On 2026-09-30 that was 680 armed TST rows — work whose acceptance bar is "it
-- compiles" — each paying a Scout, an Architect and a Lead turn, and one of the first four arms died
-- in the Architect turn and took the story with it.
--
-- WHY A COLUMN AND NOT A WIDER `kind`. The two vocabularies answer two different questions and 179's
-- constraint is honest about the batch's; widening it would make one column mean two things. This is
-- the shape 179 itself chose for `model_policy`: the answer is copied onto the queue row at the
-- moment it is known, so the run is configured by the ROW and not by argv.
--
-- WHO WRITES IT. The story row declares it (`storyboard_story.work_type`); the Ready trigger — the
-- one writer of `agent_work_item` rows (025, last replaced in 146, whose ON CONFLICT arbiter is kept
-- here byte for byte, because it is what makes inference against migration 143's index work) — copies
-- it onto the item it queues.
--
-- NULL IS A MEANING, not a missing value: an undeclared story is FEATURE, which is what every row did
-- before this migration. No board row is relabelled here: declaring the TST backlog's work type is a
-- data decision and travels in its own reversible load
-- (`db/loads/declare_tst_fast_lane_2026_09_30.sql`).
--
-- NOTHING IS WEAKENED. Declaring FAST does not grant the lane: `forge_fast_eligibility`
-- (`forge/src/engine/facts.rs:247`) still refuses it for a story that needs a migration, a
-- derived refresh, a deployment, an architecture change, or one the Lead split. This column chooses
-- which work type the run is DISPATCHED as; the graph still decides what it may do.
--
-- Non-destructive: two added nullable columns, two check constraints, one index, one function
-- replaced.

begin;

alter table storyboard_story
    add column if not exists work_type text null;

alter table storyboard_story
    drop constraint if exists storyboard_story_work_type_check,
    add constraint storyboard_story_work_type_check
        check (work_type is null or work_type in
               ('FEATURE', 'FAST', 'BUG', 'HOTFIX', 'RESEARCH', 'MIGRATION'));

alter table agent_work_item
    add column if not exists work_type text null;

alter table agent_work_item
    drop constraint if exists agent_work_item_work_type_check,
    add constraint agent_work_item_work_type_check
        check (work_type is null or work_type in
               ('FEATURE', 'FAST', 'BUG', 'HOTFIX', 'RESEARCH', 'MIGRATION'));

-- "What is queued fast right now?" — the same question 179's routing index answers for kind.
create index if not exists agent_work_item_work_type_idx on agent_work_item (work_type)
    where state in ('Ready', 'Claimed', 'Running');

create or replace function agent_work_item_dispatch() returns trigger
language plpgsql as $$
begin
    if new.status = 'Ready' and (tg_op = 'INSERT' or old.status is distinct from 'Ready') then
        insert into agent_work_item (story_id, state, priority, work_type)
        values (new.id, 'Ready', story_priority_score(new.priority), new.work_type)
        on conflict (story_id)
            where state in ('Ready', 'Claimed', 'Running', 'Paused')
              and parallel_group_id is null
            do nothing;
    end if;
    return new;
end;
$$;

commit;
