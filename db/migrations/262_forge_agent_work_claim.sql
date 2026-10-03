-- 262_forge_agent_work_claim.sql
--
-- WHY: the two queue claims were transactions choreographed from Rust (`ForgeEngineDao::claim_specific_agent_work`
-- and `claim_next_agent_work`, db/src/forge_engine.rs): open a transaction, take the claim lock, read,
-- decide, compare-and-set, commit. That is database behaviour, so it lives in the database now and the Rust DAO
-- binds two parameters and maps the row. Translated as-is: no new column, no new queue, no change to ordering,
-- eligibility or attempts.
--
-- TWO ROUTINES, NOT ONE. The claims differ in more than "is an id supplied": the specific claim refuses when the
-- item's story already has an open item (and, for a split slot, only a serial one), and does not read the board;
-- the next claim reads the board (`storyboard_story.status = 'Ready'`) and the unattended policy, orders the
-- queue, skips locked rows, and leaves "one writer per story" to the unique indexes
-- `agent_work_item_one_serial_active_per_story` / `agent_work_item_one_parallel_slot` (a collision is a unique
-- violation raised to the caller, as before).
--
-- THE CLAIM LOCK. `pg_advisory_xact_lock(9000212)` is the key the Rust constant `AGENT_CLAIM_LOCK` carried. It
-- serializes claim SELECTION and is released when the caller's transaction ends — for a plain `select` of these
-- functions that is the end of the statement, exactly where the Rust transaction committed.
--
-- Each statement inside runs on its own snapshot (plpgsql, READ COMMITTED), as each Rust statement did, so the
-- reads after the lock see every claim committed before it was granted.
--
-- Both return the claimed `agent_work_item` row, or no row when nothing was claimed.

begin;

create or replace function forge_claim_specific_agent_work(p_work_item_id uuid, p_worker_id text)
returns setof agent_work_item
language plpgsql
as $$
declare
    v_group uuid;
begin
    perform pg_advisory_xact_lock(9000212);

    select w.parallel_group_id into v_group
      from agent_work_item w
     where w.id = p_work_item_id;

    -- An open item on the same story refuses the claim. A split slot is refused only by a SERIAL open item:
    -- its sibling slots are the parallel work it is part of.
    if exists (
        select 1
          from agent_work_item a
         where a.state in ('Claimed', 'Running', 'Paused')
           and (v_group is null or a.parallel_group_id is null)
           and a.story_id = (select w.story_id from agent_work_item w where w.id = p_work_item_id)
    ) then
        return;
    end if;

    return query
    update agent_work_item w
       set state = 'Claimed', claimed_at = now(), claimed_by = p_worker_id,
           attempts = w.attempts + 1, updated_at = now()
     where w.id = p_work_item_id
       and w.state = 'Ready'
    returning w.*;
end;
$$;

create or replace function forge_claim_next_agent_work(p_worker_id text)
returns setof agent_work_item
language plpgsql
as $$
begin
    perform pg_advisory_xact_lock(9000212);

    -- Eligible: the item is Ready, its story is still Ready on the board (a story being worked is never
    -- re-dispatched), and its policy allows the unattended poller (migration 029). `skip locked` keeps the
    -- statement correct for a caller outside the claim lock; the update re-checks `state = 'Ready'`.
    return query
    with candidate as (
        select w.id
          from agent_work_item w
          join storyboard_story s on s.id = w.story_id
         where w.state = 'Ready' and s.status = 'Ready'
           and w.execution_policy = 'Unattended OK'
         order by w.priority desc, w.queued_at asc, w.id
           for update of w skip locked
         limit 1
    )
    update agent_work_item w
       set state = 'Claimed', claimed_at = now(), claimed_by = p_worker_id,
           attempts = w.attempts + 1, updated_at = now()
      from candidate c
     where w.id = c.id
       and w.state = 'Ready'
    returning w.*;
end;
$$;

commit;
