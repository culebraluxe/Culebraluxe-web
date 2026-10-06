-- 275_forge_claim_story_brake.sql
--
-- WHY. Four things are true at once after 274 and no routine honours any of them.
--
--   1. `forge_runtime_control.paused` is written by a setter and read by nobody, so `forge_set_paused(true)` stops
--      nothing: the worker keeps claiming from the work item queue on every tick (274:19-22 promised the guard here).
--   2. `global_story_concurrency` is a ceiling no claim reads. The worker's own slot count is decided by an
--      environment variable on whichever host runs it, so the ceiling an operator sets in the database - the one
--      fact every executor shares - is decorative.
--   3. The claim door is `forge_claim_next_agent_work(p_worker_id)` (262): one item, no ceiling read, no brake.
--   4. Staleness has one rule - an item untouched for `stale_after_minutes` (Rust
--      `ForgeControlDao::stale_agent_work`, db/src/forge_control.rs:39-54) - and it is the wrong rule for a claim
--      held by a worker. A worker that is alive but slow (a long model turn) looks identical to a worker that is
--      dead, and 273 introduced the row that tells them apart: `forge_worker_health.stale`.
--
-- WHAT IT IS. Four routines, one of them new as the door.
--
--   forge_claim_story(worker_id, max)      THE door: refuses while paused, honours the global ceiling, claims
--                                          `least(max, ceiling - running)` items oldest-first, rings 'claimed'
--   forge_claim_next_agent_work(worker_id) now a delegation to `forge_claim_story(p_worker_id, 1)` - one
--                                          implementation, so the old name cannot drift into a second door
--   forge_arm_work_queue(limit, by)        272's body plus the same pause guard: paused arms nothing
--   forge_reapable_claims(stale_after)     the one staleness rule, in one place: the age fallback OR a stale worker
--
-- THE BRAKE IS A REFUSAL, NOT AN EXCEPTION. Paused returns an empty set, with one `warning` line naming who paused
-- it. An exception would make a deliberate brake look like a fault in every caller's log, and a worker that reads
-- `paused` for itself never reaches the warning at all.
--
-- THE CEILING IS READ UNDER THE CLAIM LOCK. `pg_advisory_xact_lock(9000212)` is 262's constant - the same mutex,
-- not a second one - so two workers cannot both read `running` and both spend the same free slot. The count is over
-- DISTINCT stories in ('Claimed','Running','Paused'): a story is in flight if any item of it is held, and a `Paused`
-- item is held by definition (a human has it), so it must not be silently handed a second slot.
--
-- A PAUSED DOOR IS NOT A STOPPED QUEUE. Everything 274 promised stays true: work already in flight finishes and
-- settles normally (the settle and its event arm both run; only the arm's pace is refused here), and rows already
-- armed keep their slots. This routine gates the claim - the second of the two doors 274:19-22 names.
--
-- REAPING FOLLOWS THE WORKER, NOT THE CLOCK. `forge_reapable_claims` returns an item when the claim is old (the
-- fallback that already existed, kept for a claim whose worker never beat - a one-shot run, a test, an operator's
-- manual claim) OR when the worker that holds it is `stale` in `forge_worker_health` (273). The second rule is the
-- new one and it is the worker's own evidence: `last_seen_at` is written by the worker about itself. Both rules
-- live here rather than in the Rust caller, so one place answers "is this claim reapable" and the Rust side reads a
-- list instead of re-deciding.
--
-- GRANTS. The executable surface is not the connection owner. If a least-privilege `forge_worker` role exists, it
-- needs exactly the reads and the two write doors below; the block is conditional because that role does not exist
-- on DEV or PROD today (verified 2026-10-06: neither target has any `forge*` role) and creating a database role
-- with a password is a decision for the operator, not a side effect of a migration.
--
-- REVERSAL. `drop function forge_reapable_claims(integer);` then re-run `262_forge_agent_work_claim.sql` for the
-- claim door and `272_forge_work_queue_event_payload.sql` for `forge_arm_work_queue`; both files carry complete
-- previous definitions. `forge_claim_story` is new here and has no earlier form to restore.
--
-- Non-destructive: three routines replaced, two added. No table, column, constraint, index, status or reason text is
-- read or written differently, and no row is moved by this file: it is the brake, not the traffic.

begin;

-- ============================================================
-- THE CLAIM DOOR - paused refuses, the ceiling caps, oldest wins.
-- ============================================================

create or replace function forge_claim_story(
    p_worker_id text,
    p_max integer default 1
)
returns setof agent_work_item
language plpgsql
as $$
declare
    v_paused boolean;
    v_paused_by text;
    v_ceiling integer;
    v_running integer;
    v_slots integer;
    v_candidate record;
    v_claimed agent_work_item;
begin
    if btrim(coalesce(p_worker_id, '')) = '' then
        raise exception 'forge_claim_story: worker_id is blank (a claim nobody owns is a claim nobody can reap)'
            using errcode = '22023';
    end if;
    if p_max is null or p_max < 1 then
        raise exception 'forge_claim_story: p_max must be at least 1 (asked for %)', p_max
            using errcode = '22023';
    end if;

    -- ONE MUTEX, 262's CONSTANT. The ceiling is read-then-spent, so it must be read under the same lock every other
    -- claim takes: without it two workers read `running = 1`, both conclude a slot is free and both take one.
    perform pg_advisory_xact_lock(9000212);

    select c.paused, c.updated_by, c.global_story_concurrency
      into v_paused, v_paused_by, v_ceiling
      from forge_runtime_control c
     where c.id = 1;

    if not found then
        -- 274 creates the row with the table, so a missing row means the brake is unknown. Fail closed: a door that
        -- cannot see the brake must not open.
        raise exception 'forge_claim_story: forge_runtime_control row 1 is missing (no brake to read)'
            using errcode = 'P0001';
    end if;

    if v_paused then
        raise warning 'forge_claim_story: paused by % - no claim taken', coalesce(v_paused_by, 'unknown');
        return;
    end if;

    -- Stories in flight. DISTINCT, because a parallel group holds several items of one story and that is still one
    -- story against the ceiling.
    select count(distinct w.story_id) into v_running
      from agent_work_item w
     where w.state in ('Claimed', 'Running', 'Paused');

    v_slots := least(p_max, v_ceiling - v_running);
    if v_slots <= 0 then
        return;
    end if;

    -- 262's eligibility and 262's order, unchanged: the item is Ready, its story is still Ready on the board (a
    -- story being worked is never re-dispatched), and the policy allows the unattended poller (migration 029).
    for v_candidate in
        select w.id
          from agent_work_item w
          join storyboard_story s on s.id = w.story_id
         where w.state = 'Ready' and s.status = 'Ready'
           and w.execution_policy = 'Unattended OK'
         order by w.priority desc, w.queued_at asc, w.id
           for update of w skip locked
         limit v_slots
    loop
        update agent_work_item w
           set state = 'Claimed', claimed_at = now(), claimed_by = p_worker_id,
               attempts = w.attempts + 1, updated_at = now()
         where w.id = v_candidate.id
           and w.state = 'Ready'
        returning w.* into v_claimed;

        if found then
            -- The doorbell for a claim names the STORY and leaves `id` null: this is an item event, and the queue
            -- row a claim may later be armed from does not exist yet. An item id in the queue-id slot would be a lie
            -- about what that id is.
            perform forge_notify_work('claimed', null, v_claimed.story_id);
            return next v_claimed;
        end if;
    end loop;

    return;
end;
$$;

comment on function forge_claim_story(text, integer) is
    'The claim door: refuses while forge_runtime_control.paused, honours global_story_concurrency, claims oldest-first.';

-- ============================================================
-- THE OLD NAME - a delegation, so there is one claim implementation.
-- ============================================================

create or replace function forge_claim_next_agent_work(p_worker_id text)
returns setof agent_work_item
language sql
as $$
    select * from forge_claim_story(p_worker_id, 1);
$$;

comment on function forge_claim_next_agent_work(text) is
    '262''s name, kept for existing callers: the mutex, the brake and the ceiling all live in forge_claim_story.';

-- ============================================================
-- THE SECOND DOOR - 272's arm, with the read that makes `paused` mean something.
-- ============================================================

create or replace function forge_arm_work_queue(
    p_limit integer default null,
    p_armed_by text default 'forge-sweep'
)
returns bigint
language plpgsql
as $$
declare
    v_limit integer;
    v_inflight bigint;
    v_paused boolean;
    v_paused_by text;
    v_row record;
    v_outcome text;
    v_armed bigint := 0;
begin
    -- THE BRAKE, AT THE SECOND DOOR (274:19-22). Rows already `Running` keep their slots and their runs finish;
    -- what stops is handing a free slot to the next `Pending` row.
    select c.paused, c.updated_by into v_paused, v_paused_by from forge_runtime_control c where c.id = 1;
    if not found then
        -- Fail closed: an arm that cannot see the brake must not move work.
        raise warning 'forge_arm_work_queue: forge_runtime_control row 1 is missing - nothing armed';
        return 0;
    end if;
    if v_paused then
        raise warning 'forge_arm_work_queue: paused by % - nothing armed', coalesce(v_paused_by, 'unknown');
        return 0;
    end if;

    select coalesce(p_limit, c.arm_limit) into v_limit from forge_work_queue_config c where c.id = 1;
    v_limit := coalesce(v_limit, 3);
    if v_limit < 1 then
        return 0;
    end if;

    -- Pacing: only the slots nobody holds. This is what keeps the first pass of a tick from arming the load.
    select count(*) into v_inflight from forge_work_queue q where q.state = 'Running';
    if v_inflight >= v_limit then
        return 0;
    end if;

    for v_row in
        select q.id, q.story_id
          from forge_work_queue q
          join storyboard_story s on s.id = q.story_id
         where q.state = 'Pending'
           and q.available_at <= now()
           and q.attempts < q.max_attempts
           and s.status in ('Planned', 'Ready')
           and not exists (
                 select 1 from agent_work_item w
                  where w.story_id = q.story_id
                    and w.state in ('Ready', 'Claimed', 'Running', 'Paused')
                    and w.parallel_group_id is null)
         order by q.priority asc, q.created_at asc, q.id asc
           for update of q skip locked
         limit v_limit - v_inflight
    loop
        update forge_work_queue q
           set state = 'Running',
               claimed_by = p_armed_by,
               claimed_at = now(),
               started_at = coalesce(q.started_at, now()),
               attempts = q.attempts + 1,
               updated_at = now()
         where q.id = v_row.id;

        -- The database's own dispatch verb. `AlreadyQueued` is a confirmation, not a failure: it means the story
        -- already holds the one open item, which is the state this arm is trying to reach.
        select d.outcome into v_outcome from forge_dispatch_story(v_row.story_id) d;
        if v_outcome = 'Missing' then
            perform forge_fail_work(v_row.id, format('story %s no longer exists', v_row.story_id));
        else
            v_armed := v_armed + 1;
            perform forge_notify_work('armed', v_row.id, v_row.story_id);
        end if;
    end loop;

    return v_armed;
end;
$$;

-- ============================================================
-- STALENESS - two reasons, one rule, one place.
-- ============================================================

create or replace function forge_reapable_claims(p_stale_after_minutes integer)
returns table (
    id uuid,
    story_id text,
    role text,
    attempts integer,
    max_attempts integer,
    story_run_id uuid,
    updated_at timestamptz,
    why text
)
language sql
stable
as $$
    select w.id,
           w.story_id,
           w.role,
           w.attempts,
           coalesce(w.max_attempts, 3) as max_attempts,
           w.story_run_id,
           w.updated_at,
           case
               when coalesce(h.stale, false)
                   then format('worker %s last beat %s seconds ago', w.claimed_by, h.age_seconds)
               else format('claim untouched for %s minutes (no worker heartbeat for this claim)',
                           greatest(coalesce(p_stale_after_minutes, 0), 0))
           end as why
      from agent_work_item w
      left join forge_worker_health h on h.worker_id = w.claimed_by
     where w.state in ('Claimed', 'Running', 'Paused')
       and (
            w.updated_at < now()
                - (greatest(coalesce(p_stale_after_minutes, 0), 0)::text || ' minutes')::interval
            or coalesce(h.stale, false)
       )
     order by w.updated_at asc;
$$;

comment on function forge_reapable_claims(integer) is
    'Claims a worker has walked away from: the claim is older than the window, or the worker holding it is stale in forge_worker_health.';

-- ============================================================
-- GRANTS - only if the least-privilege role exists. It does not, on DEV or PROD, today.
-- ============================================================

do $$
begin
    if exists (select 1 from pg_roles where rolname = 'forge_worker') then
        grant select on forge_runtime_control, forge_worker_heartbeat, forge_worker_health,
                        agent_work_item, storyboard_story, forge_work_queue, forge_work_queue_config
          to forge_worker;
        grant execute on function forge_claim_story(text, integer),
                                forge_claim_next_agent_work(text),
                                forge_begin_agent_work_run(uuid, text),
                                forge_finish_agent_work_run(uuid, text, text),
                                forge_worker_beat(text, text, text, text, integer, text, timestamptz)
          to forge_worker;
        raise notice 'forge_worker granted: reads on the control plane, the claim, the begin, the settle and the beat';
    else
        raise notice 'no forge_worker role on this target: grants skipped (create the role, then re-run this block)';
    end if;
end $$;

commit;
