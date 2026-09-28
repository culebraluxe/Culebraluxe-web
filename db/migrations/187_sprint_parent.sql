-- 187_sprint_parent.sql
--
-- A SPRINT BECOMES A ROW, NOT JUST A NUMBER ON THE STORY (Captain, 2026-09-17:
-- "I am thinking sprints is how I am going to run this going forwards").
--
-- Until now the sprint was `storyboard_story.batch` — an integer, and nothing else. That integer is
-- real and load-bearing: the engine reads it, the release story defers deployment to it, the
-- storyboard orders by it. But a number cannot carry a goal, a status, an owner, a start date, or a
-- close date, and it cannot state what a sprint was FOR. So every sprint we have run is
-- reconstructed from its stories after the fact, and the things a sprint is judged by — its goal,
-- and what actually happened — were never recorded anywhere.
--
-- This migration adds the PARENT without disturbing the axis the engine already uses:
--
--   * `storyboard_sprint` — one row per sprint. `number` is the batch axis, so a sprint and a batch
--     are the same integer, spelled once.
--   * `storyboard_story.sprint_id` — the child link, DERIVED from `batch` by trigger. It is never
--     hand-maintained and never silently reconciled: a write that disagrees with the batch is
--     REFUSED by name, because a link that can drift is worse than no link.
--   * `storyboard_story.carried_over_from_sprint_id` — where a story came from when a sprint did not
--     finish it, so carry-over is a recorded fact instead of a re-batching nobody can explain later.
--
-- Two rules are encoded rather than documented:
--
--   1. AN UNBATCHED STORY IS UNASSIGNED, NEVER INVENTED INTO A SPRINT. 283 stories carry a null
--      batch today; they stay null. Absent stays absent, exactly as `acceptance_assertions` does
--      (186).
--   2. A CLOSED SPRINT SAYS WHAT ACTUALLY HAPPENED. `status = 'Closed'` requires `closed_at` and
--      `outcome`, the same way `storyboard_story_completion_requires_complete` (182) requires a
--      Complete story to be at 100. A sprint that cannot say what happened is not closed, and the
--      database will not let it claim otherwise.
--
-- Backfill is derived and labelled as derived: sprints are created for the batches that exist, and
-- their status/outcome are computed from their stories AT MIGRATION TIME and say so in `notes`, so
-- no one later mistakes a backfilled row for a human declaration.
--
-- Non-destructive: one new table, two nullable added columns, three views, one trigger. No existing
-- column, row or constraint is touched, and `batch` keeps working exactly as it does today.

create table if not exists storyboard_sprint (
    id           text primary key,
    number       integer not null unique,
    title        text not null,
    theme        text,
    goal         text,
    status       text not null default 'Planned',
    owner        text,
    started_at   timestamptz,
    target_end_at timestamptz,
    closed_at    timestamptz,
    outcome      text,
    notes        text,
    created_at   timestamptz not null default now(),
    updated_at   timestamptz not null default now()
);

-- THE CONSTRAINTS ARE ADDED OUTSIDE THE `create table` BODY, AND ON PURPOSE. `create table if not
-- exists` is a no-op when the table already exists, so a constraint written only inside the body
-- would never reach a table created by an earlier version of this file — the shape would stay old
-- and silently drift from a fresh install. Named, guarded blocks make a re-apply add what is
-- missing instead of trusting that it was there.
do $$
begin
    if not exists (select 1 from pg_constraint where conname = 'storyboard_sprint_number_non_negative') then
        alter table storyboard_sprint
            add constraint storyboard_sprint_number_non_negative check (number >= 0);
    end if;
    if not exists (select 1 from pg_constraint where conname = 'storyboard_sprint_status_known') then
        alter table storyboard_sprint
            add constraint storyboard_sprint_status_known
            check (status in ('Planned', 'Active', 'Closing', 'Closed', 'Cancelled'));
    end if;
    -- THIS ONE RULE IS DROP-AND-RE-ADD, NOT ADD-IF-MISSING. The first version of it reached dev (and
    -- the same name), so an add-if-missing guard would keep the broken one forever on any plane that
    -- had applied the earlier file. Dropping by name and re-adding leaves the same end state on every
    -- plane whether or not it saw the earlier version.
    if exists (select 1 from pg_constraint where conname = 'storyboard_sprint_closed_says_what_happened') then
        alter table storyboard_sprint drop constraint storyboard_sprint_closed_says_what_happened;
    end if;
    -- `coalesce(..., 0) > 0` RATHER THAN `outcome is not null`, because a CHECK constraint treats
    -- NULL as SATISFIED. The first version of this rule read
    --   `status <> 'Closed' or (closed_at is not null and outcome is not null)`
    -- which evaluates to NULL — and therefore PASSES — when a sprint is set to Closed with both
    -- fields left empty, i.e. the rule could not stop the exact write it exists to stop. Written
    -- so every branch is true or false and never NULL, it refuses that write (measured on dev
    -- 2026-09-17 by probing it: the first version let `status = 'Closed'` through untouched).
    alter table storyboard_sprint
        add constraint storyboard_sprint_closed_says_what_happened
        check (
            status <> 'Closed'
            or (
                closed_at is not null
                and coalesce(length(btrim(outcome)), 0) > 0
            )
        );
end $$;

comment on table storyboard_sprint is
    'One row per sprint: the parent of the stories whose batch equals its number. The goal is what the sprint is FOR; the outcome is what actually happened, required at close (187).';
comment on column storyboard_sprint.number is
    'The batch axis, spelled once. storyboard_story.batch is the engine-facing side of this same integer.';
comment on column storyboard_sprint.goal is
    'What closing this sprint means. An unstated goal makes a close unjudgeable, so record it before starting.';
comment on column storyboard_sprint.outcome is
    'What actually happened, in the captain''s words, required by the check constraint when status = Closed.';


alter table storyboard_story
    add column if not exists sprint_id text,
    add column if not exists carried_over_from_sprint_id text;

do $$
begin
    if not exists (
        select 1 from pg_constraint where conname = 'storyboard_story_sprint_fk'
    ) then
        alter table storyboard_story
            add constraint storyboard_story_sprint_fk
            foreign key (sprint_id) references storyboard_sprint (id) on delete set null;
    end if;
    if not exists (
        select 1 from pg_constraint where conname = 'storyboard_story_carried_from_fk'
    ) then
        alter table storyboard_story
            add constraint storyboard_story_carried_from_fk
            foreign key (carried_over_from_sprint_id) references storyboard_sprint (id) on delete set null;
    end if;
end $$;

comment on column storyboard_story.sprint_id is
    'The sprint this story belongs to, derived from batch by trigger. Never hand-maintained: a write that disagrees with batch is refused by name (187).';
comment on column storyboard_story.carried_over_from_sprint_id is
    'The sprint that did not finish this story, when it moved. Carry-over is a recorded fact, not a re-batching someone has to reconstruct (187).';

create index if not exists storyboard_story_sprint_idx
    on storyboard_story (sprint_id);
create index if not exists storyboard_sprint_status_idx
    on storyboard_sprint (status);

-- THE LINK IS DERIVED, AND A DISAGREEMENT IS LOUD. `S<batch>` is the only spelling, so the two
-- sides of the same integer cannot drift: a null batch leaves the story unassigned, an insert takes
-- its sprint from its batch, a re-batch carries the story with it unless the sprint was changed
-- explicitly in the same write, and anything else raises with both values named.
create or replace function storyboard_sprint_id_for_batch(p_batch integer)
returns text language sql immutable as $$
    select case when p_batch is null then null else 'S' || p_batch::text end
$$;

create or replace function storyboard_story_sprint_link()
returns trigger language plpgsql as $$
declare
    expected text;
begin
    if new.batch is null then
        return new;  -- unassigned, never invented into a sprint
    end if;
    expected := storyboard_sprint_id_for_batch(new.batch);
    if new.sprint_id is null then
        new.sprint_id := expected;
    elsif tg_op = 'UPDATE'
          and new.batch is distinct from old.batch
          and new.sprint_id is not distinct from old.sprint_id then
        new.sprint_id := expected;  -- a re-batch carries the story unless the sprint was set too
    elsif new.sprint_id <> expected then
        raise exception
            'storyboard_story % would sit in sprint % while its batch % says %; change both, or neither',
            new.id, new.sprint_id, new.batch, expected
            using errcode = 'check_violation';
    end if;
    -- A SPRINT MUST EXIST BEFORE STORIES LAND IN IT, and the refusal says so by name: the foreign key
    -- would catch this anyway, but as 'insert violates constraint' with no way to see what is missing.
    if not exists (select 1 from storyboard_sprint where id = new.sprint_id) then
        raise exception
            'batch % has no storyboard_sprint row (%); create the sprint before batching stories into it',
            new.batch, new.sprint_id
            using errcode = 'foreign_key_violation';
    end if;
    return new;
end
$$;

comment on function storyboard_story_sprint_link() is
    'Keeps storyboard_story.sprint_id equal to S<batch>. Refuses a disagreement by name rather than reconciling it silently (187).';

drop trigger if exists storyboard_story_sprint_link on storyboard_story;
create trigger storyboard_story_sprint_link
    before insert or update of batch, sprint_id on storyboard_story
    for each row execute function storyboard_story_sprint_link();

-- BACKFILL, LABELLED AS BACKFILL. Every batch that already exists gets a row, and the row says in
-- `notes` that its status and dates were COMPUTED from its stories at migration time rather than
-- declared. A closed backfilled sprint carries a computed outcome for the same reason: the database
-- rule demands one, and the honest thing to write is that no human wrote it.
with per_batch as (
    select
        batch,
        count(*) as stories,
        count(*) filter (where status = 'Complete') as done,
        min(created_at) as first_story_at,
        max(completed_at) as last_completed_at
    from storyboard_story
    where batch is not null
    group by batch
)
insert into storyboard_sprint (id, number, title, status, started_at, closed_at, outcome, notes)
select
    'S' || b.batch::text,
    b.batch,
    'Sprint ' || b.batch::text,
    case
        when b.done = b.stories and b.last_completed_at is not null then 'Closed'
        when b.done > 0 then 'Active'
        else 'Planned'
    end,
    b.first_story_at,
    case when b.done = b.stories and b.last_completed_at is not null then b.last_completed_at end,
    case
        when b.done = b.stories and b.last_completed_at is not null then
            b.done::text || ' of ' || b.stories::text || ' stories Complete when this row was backfilled; ' ||
            'no outcome was declared at the time, so this line is computed, not written by a human.'
    end,
    'Backfilled from batch ' || b.batch::text || ' by migration 187. Status, start and close are DERIVED from ' ||
    'its stories at migration time, not declared. Set goal before working it; set outcome before closing it.'
from per_batch b
on conflict (id) do nothing;

-- The children finally resolve their parent. Runs after the backfill so no story points at a sprint
-- that does not exist yet.
update storyboard_story
set sprint_id = storyboard_sprint_id_for_batch(batch)
where batch is not null
  and sprint_id is distinct from storyboard_sprint_id_for_batch(batch);

-- WHAT THE BOARD WILL ASK, ANSWERED IN SQL INSTEAD OF IN A SCRIPT.
create or replace view storyboard_sprint_rollup as
select
    s.id,
    s.number,
    s.title,
    s.theme,
    s.status,
    s.owner,
    s.goal,
    s.started_at,
    s.target_end_at,
    s.closed_at,
    s.outcome,
    count(st.id) as stories,
    count(st.id) filter (where st.status = 'Complete') as stories_complete,
    count(st.id) filter (where st.status in ('Planned', 'Ready', 'Batched', 'Deferred', 'In Progress')) as stories_open,
    count(st.id) filter (where st.status = 'Hold') as stories_held,
    case
        when count(st.id) = 0 then null
        else round(100.0 * count(st.id) filter (where st.status = 'Complete') / count(st.id), 1)
    end as percent_complete,
    min(st.completed_at) as first_completed_at,
    max(st.completed_at) as last_completed_at
from storyboard_sprint s
left join storyboard_story st on st.sprint_id = s.id
group by s.id, s.number, s.title, s.theme, s.status, s.owner, s.goal, s.started_at, s.target_end_at,
         s.closed_at, s.outcome;

comment on view storyboard_sprint_rollup is
    'One row per sprint with its children counted, including holds as their own number. percent_complete is null, never 0, for a sprint with no stories (187).';

create or replace view storyboard_sprint_story as
select
    st.id as story_id,
    st.title as story_title,
    st.status as story_status,
    st.priority,
    st.completion,
    st.sprint_id,
    s.number as sprint_number,
    s.title as sprint_title,
    s.status as sprint_status,
    st.carried_over_from_sprint_id,
    st.completed_at
from storyboard_story st
join storyboard_sprint s on s.id = st.sprint_id;

comment on view storyboard_sprint_story is
    'Each story with its sprint context, for the board. Only linked stories appear; the 283 unbatched stories are deliberately absent because they belong to no sprint (187).';

