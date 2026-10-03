-- 265_forge_dispatch_reconcile.sql
--
-- WHY: putting a story into the engine's queue (`dispatch_story_in`, behind `ensure_story_dispatched` — the board's
-- ENGINE RUN Q move and the `captureCommit` scoping path) and the control-plane sweep that runs before every run
-- (`ForgeEngineDao::reconcile_dispatch_queue`) were transactions choreographed from Rust
-- (rust/core/db/src/forge_engine.rs). The sweep calls the dispatch verb, so they move together: one spelling of
-- "make the database dispatch this story" for the board and for the sweep. Translated as-is — the fourth
-- stored-routine slice after 262 (claim), 263 (settlement) and 264 (run open). No change to the trigger, the
-- score, the arbiter, any status or any guard.
--
-- DISPATCH OWNS NO RULE. `agent_work_item_dispatch()` (025, restated in 146) owns the whole of dispatch: it inserts
-- the one item, scores it and lets the partial unique index arbitrate. `forge_dispatch_story` only restores the
-- CHANGE into `Ready` the trigger fires on — off `Ready` and back, as two statements, because a data-modifying CTE
-- shares one snapshot and could not fire it — and returns the item the trigger wrote. The story is locked while its
-- status and its slot are read together. A story that already holds an open slot is confirmed, not duplicated.
--
-- No item after the change means the database and the deployment disagree about the schema (a missing or renamed
-- trigger). That is raised as SQLSTATE 42704 (undefined_object), which rolls the status change back, and the DAO
-- types it as `SchemaMismatch` — refused and captured, never "nothing to do".
--
-- THE SWEEP repairs three shapes, in one transaction, in this order, and never touches a live run (a
-- `Claimed`/`Running` item or an active process instance):
--   1. a story `In Progress` that Forge demonstrably owned (an open `Ready`/`Paused` item or an unclosed run) with
--      nothing holding it goes back to `Ready`. A story with no such evidence is a human's OPEN card and is left
--      exactly as it is: reconciliation may repair Forge state, never invent authorization;
--   2. a `Ready` story with no open serial slot is handed to `forge_dispatch_story`, the board's own verb;
--   3. an open item whose story no longer expects a run is cleared to `Cancelled`, saying why.

begin;

-- `outcome` is `Queued` (the trigger created `item`), `AlreadyQueued` (the story held `item`) or `Missing`.
create or replace function forge_dispatch_story(p_story_id text)
returns table (outcome text, item text)
language plpgsql
as $$
declare
    v_status text;
    v_item text;
begin
    select s.status into v_status from storyboard_story s where s.id = p_story_id for update;
    if not found then
        outcome := 'Missing';
        return next;
        return;
    end if;

    select w.id::text into v_item
      from agent_work_item w
     where w.story_id = p_story_id
       and w.state in ('Ready', 'Claimed', 'Running', 'Paused')
       and w.parallel_group_id is null
     order by w.queued_at desc
     limit 1;
    if v_item is not null then
        outcome := 'AlreadyQueued';
        item := v_item;
        return next;
        return;
    end if;

    if v_status = 'Ready' then
        update storyboard_story s set status = 'Planned', updated_at = now() where s.id = p_story_id;
    end if;
    update storyboard_story s set status = 'Ready', updated_at = now() where s.id = p_story_id;

    select w.id::text into v_item
      from agent_work_item w
     where w.story_id = p_story_id
       and w.state in ('Ready', 'Claimed', 'Running', 'Paused')
       and w.parallel_group_id is null
     order by w.queued_at desc
     limit 1;
    if v_item is null then
        raise exception using
            errcode = '42704',
            message = format(
                'the story %s is `Ready` and the Ready changed, but `agent_work_item_dispatch()` created no work '
                'item: the trigger this deployment relies on is missing or no longer fires on a change into `Ready`',
                p_story_id);
    end if;

    outcome := 'Queued';
    item := v_item;
    return next;
end;
$$;

create or replace function forge_reconcile_dispatch_queue(out queued bigint, out restated bigint, out cleared bigint)
language plpgsql
as $$
declare
    v_story text;
    v_outcome text;
begin
    update storyboard_story s
       set status = 'Ready', completed_at = null, updated_at = now()
     where s.status = 'In Progress'
       and not exists (
             select 1 from agent_work_item w
              where w.story_id = s.id and w.state in ('Claimed', 'Running'))
       and not exists (
             select 1 from process_instances p
              where p.subject_type = 'story' and p.subject_id = s.id
                and p.status in ('active', 'running', 'reserved', 'suspended'))
       and (
             exists (
               select 1 from agent_work_item w
                where w.story_id = s.id and w.state in ('Ready', 'Paused'))
             or exists (
               select 1 from storyboard_story_run r
                where r.story_id = s.id and r.ended_at is null));
    get diagnostics restated = row_count;

    queued := 0;
    for v_story in
        select s.id
          from storyboard_story s
         where s.status = 'Ready'
           and not exists (
                 select 1 from agent_work_item w
                  where w.story_id = s.id and w.state in ('Ready', 'Claimed', 'Running', 'Paused')
                    and w.parallel_group_id is null)
    loop
        select d.outcome into v_outcome from forge_dispatch_story(v_story) d;
        if v_outcome = 'Queued' then
            queued := queued + 1;
        end if;
    end loop;

    update agent_work_item w
       set state = 'Cancelled', finished_at = now(), updated_at = now(),
           error_text = 'cleared: story status ' ||
             coalesce((select s.status from storyboard_story s where s.id = w.story_id), '(no story row)') ||
             ' does not expect a run'
     where w.state in ('Ready', 'Paused')
       and not exists (
             select 1 from storyboard_story s
              where s.id = w.story_id and s.status in ('Ready', 'In Progress'));
    get diagnostics cleared = row_count;
end;
$$;

commit;
