-- 270_forge_work_queue_inlet.sql
--
-- WHY. 269 built the door and nothing walked through it: a Pending row could be claimed and settled by hand, but
-- no path existed from "a row exists" to "Forge runs that story". This migration adds the two steps that make the
-- queue an inlet rather than a table — ARM (a Pending row hands its story to the database's own dispatch verb) and
-- SETTLE (the row follows the story it queued, and is never a second verdict on it).
--
-- WHERE IT LIVES. Both steps are called from `forge_reconcile_dispatch_queue()` (265) — the sweep the worker already
-- runs once per pass, before the claim (`forge/src/engine/worker.rs:231`, `db/src/forge_engine.rs:398`). The
-- routine's signature and its three repairs are unchanged, so there is no Rust change, no rebuilt binary and no
-- deploy: the sweep the engine already calls becomes the inlet, and it runs against PROD where the worker runs.
--
-- THE RUN IS UNCHANGED. Arming invents no path. It calls `forge_dispatch_story(story_id)` — the board's own
-- ENGINE RUN Q verb — so the story goes `Ready`, `agent_work_item_dispatch()` (025, restated in 146 and 259)
-- queues the one item, and from there claim (262), run open (264), settlement (263) and stale recovery (266) run
-- exactly as they do for a story armed by hand. `agent_work_item.execution_policy` defaults to 'Unattended OK',
-- which is the policy the unattended poller claims on, so an armed story is runnable with nothing added.
--
-- PACING. `forge_work_queue_config.arm_limit` (default 3) is the whole rate control: the sweep arms only until that
-- many door rows are `Running`, so a 573-row load is a queue, not a flood, and a pass that arms nothing is the
-- normal steady state. Raising it for a batch is one UPDATE.
--
-- THE DOOR IS PROVENANCE, NOT ROUTING. `execution_target` records what the enqueuer intended; the agent a run
-- actually uses is whatever the worker's environment resolves — exactly as before this migration, and deliberately
-- so, because the claim (262) has no target filter and the story queue has one writer per fact. `ran_as` is added
-- so the row also carries what actually ran (the run's own `model_used`), stamped at settle: declared intent and
-- observed fact stay two columns with one writer each, and neither can be read as the other. A blank target is
-- refused, because a blank one is not an intent.
--
-- RETRY POLICY. A story `Failed` is terminal at the door: the engine already owns repair inside a run, and the
-- queue must not silently spend further turns on a story Forge has given up on. A run that simply ended — the
-- story back on the board with no item and nothing holding it — returns the row to `Pending`, which is the retry
-- the engine is already making, bounded here by `attempts`/`max_attempts`.
--
-- Non-destructive: one table, one column, one check constraint, three routines replaced. No existing table,
-- column, status or guard is read differently, and 269's table, index and five routines are untouched.

begin;

-- The one knob: how many door rows may be in flight at once.
create table if not exists forge_work_queue_config (
    id smallint primary key default 1 check (id = 1),
    arm_limit integer not null default 3 check (arm_limit between 1 and 500),
    updated_at timestamptz not null default now()
);

insert into forge_work_queue_config (id) values (1)
on conflict (id) do nothing;

-- What actually ran, as the run itself recorded it (see the header: provenance, not routing).
alter table forge_work_queue add column if not exists ran_as text;

alter table forge_work_queue
    drop constraint if exists forge_work_queue_target_not_blank_check,
    add constraint forge_work_queue_target_not_blank_check
        check (btrim(execution_target) <> '');

-- ============================================================
-- ARM — hand the free slots to their stories, oldest row first.
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
    v_row record;
    v_outcome text;
    v_armed bigint := 0;
begin
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
        end if;
    end loop;

    return v_armed;
end;
$$;

-- ============================================================
-- SETTLE — the row follows its story. Returns how many rows moved.
-- ============================================================

create or replace function forge_settle_work_queue()
returns bigint
language plpgsql
as $$
declare
    v_settled bigint := 0;
    v_rows bigint;
    v_row record;
begin
    -- 1. The story finished: the row completes, carrying the run's commit and what actually ran.
    update forge_work_queue q
       set state = 'Complete',
           commit_sha = (
               select r.commit_hash from storyboard_story_run r
                where r.story_id = q.story_id and r.ended_at is not null
                order by r.ended_at desc limit 1),
           ran_as = (
               select r.model_used from storyboard_story_run r
                where r.story_id = q.story_id and r.ended_at is not null
                order by r.ended_at desc limit 1),
           completed_at = now(),
           updated_at = now()
     where q.state = 'Running'
       and exists (select 1 from storyboard_story s where s.id = q.story_id and s.status = 'Complete');
    get diagnostics v_rows = row_count;
    v_settled := v_settled + v_rows;

    -- 2. The story failed: terminal at the door, with the engine's own reason where it left one.
    for v_row in
        select q.id, q.story_id
          from forge_work_queue q
          join storyboard_story s on s.id = q.story_id
         where q.state = 'Running' and s.status = 'Failed'
    loop
        perform forge_fail_work(
            v_row.id,
            coalesce(
                (select s.forge_last_failure_reason from storyboard_story s where s.id = v_row.story_id),
                'story ' || v_row.story_id || ' reached Failed'));
        v_settled := v_settled + 1;
    end loop;

    -- 3. The run ended without settling: the story is back on the board and nothing holds it. The engine's own
    -- sweep is already retrying that story, so the row goes back to the FIFO rather than sitting `Running`.
    update forge_work_queue q
       set state = 'Pending',
           claimed_by = null,
           claimed_at = null,
           available_at = now() + interval '30 seconds',
           last_error = 'run ended without settling',
           updated_at = now()
     where q.state = 'Running'
       and exists (select 1 from storyboard_story s
                    where s.id = q.story_id and s.status in ('Planned', 'Ready'))
       and not exists (select 1 from agent_work_item w
                        where w.story_id = q.story_id
                          and w.state in ('Ready', 'Claimed', 'Running', 'Paused'))
       and not exists (select 1 from process_instances p
                        where p.subject_type = 'story' and p.subject_id = q.story_id
                          and p.status in ('active', 'running', 'reserved', 'suspended'));
    get diagnostics v_rows = row_count;
    v_settled := v_settled + v_rows;

    -- 4. Attempts spent: nothing will claim it again, so it must not keep reading as Pending.
    update forge_work_queue q
       set state = 'Error',
           last_error = coalesce(q.last_error, 'attempts exhausted'),
           completed_at = now(),
           updated_at = now()
     where q.state = 'Pending' and q.attempts >= q.max_attempts;
    get diagnostics v_rows = row_count;
    v_settled := v_settled + v_rows;

    return v_settled;
end;
$$;

-- ============================================================
-- THE SWEEP — 265's three repairs, byte for byte, wrapped by the door.
-- ============================================================

create or replace function forge_reconcile_dispatch_queue(out queued bigint, out restated bigint, out cleared bigint)
language plpgsql
as $$
declare
    v_story text;
    v_outcome text;
begin
    -- Close first, so a finished row's slot is free for the arm below in the same pass.
    perform forge_settle_work_queue();

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

    -- Open last: each row armed here is handed to `forge_dispatch_story`, the verb the loop above just used.
    perform forge_arm_work_queue(null, 'forge-sweep');
end;
$$;

commit;
