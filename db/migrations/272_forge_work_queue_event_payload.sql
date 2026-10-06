-- 272_forge_work_queue_event_payload.sql
--
-- WHY. 269 put a doorbell on the queue: `forge_enqueue_work` fires `pg_notify('forge_work_queue', <uuid>)`. It rings
-- on exactly one event, and it rings a bare id — so a listener that wakes cannot tell what happened without going
-- back to the row, and three of the four events that change what is claimable are silent: arming a row (`Pending →
-- Running`), settling one (`Running → Complete/Error/Pending`), and taking a claim. A poller that waits 180 s
-- (S2) is the only consumer today, and the server-side executor the order describes will LISTEN — for it, "the door
-- moved" is the entire message and "which door" is the useful part.
--
-- WHAT IT IS. One channel, one payload writer, four kinds.
--
--   forge_notify_work(kind, queue_id, story_id)   the only place the payload shape is written
--   kind = 'enqueued'  `forge_enqueue_work`        a new row entered the FIFO
--   kind = 'armed'     `forge_arm_work_queue`      a FIFO row took a slot and dispatched its story
--   kind = 'settled'   `forge_settle_work_queue`   a row left `Running` (complete, failed, or back to Pending)
--   kind = 'claimed'   `forge_claim_story` (275)   a story was claimed by a named worker
--
-- ONE CHANNEL, NOT TWO. The alternative was a second channel (`forge_work`) beside `forge_work_queue`. Refused: two
-- names for one door is two answers to "has the queue moved", and a listener subscribed to the wrong one is silent
-- while looking healthy — the failure mode this whole inlet exists to remove. The channel name is unchanged, so
-- nothing that already listens has to be re-pointed; only the payload grew, and there is no parser in the tree to
-- break, because 269:20-22 says the notify is a doorbell and today's only consumer polls the rows, not the channel.
--
-- THE PAYLOAD CARRIES A KIND AND AN OPTIONAL SUBJECT. `id` and `story_id` are null for a batch event (a settle pass
-- that moves several rows has no single subject, and naming one of them would be a lie about the other three).
-- `at` is UTC ISO-8601 with milliseconds: the event's own clock, so a listener can measure its lag. An unknown kind
-- is refused rather than sent — a listener that has to guess is not being woken, it is being taunted.
--
-- NOTHING IS ANSWERED FROM THE NOTIFICATION. The rows stay the only truth; a missed notify costs latency and
-- nothing else. `pg_notify` deduplicates identical payloads inside one transaction, which is why the payload carries
-- `at`: two settles in one transaction are distinguishable events rather than one, and a listener that counts its
-- wake-ups is not told a smaller number than it acted on.
--
-- REVERSAL. This file replaces three routines whose previous definitions are complete in the migrations that
-- created them: `forge_enqueue_work` in `269_forge_work_queue.sql:81-119`, `forge_arm_work_queue` in
-- `270_forge_work_queue_inlet.sql:62-126` and `forge_settle_work_queue` in
-- `271_forge_work_queue_event_settle.sql:53-157`. Re-running any of those files restores its own definition; the
-- helper below is the only thing this file adds, and `drop function forge_notify_work(text, uuid, text)` is its
-- reversal.
--
-- Non-destructive: three routines replaced, one added. No table, column, constraint, index, status, guard or reason
-- text is read or written differently, and 269's, 270's and 271's behaviour on the rows is unchanged.

begin;

-- ============================================================
-- THE PAYLOAD — one writer, so a listener has one shape to parse.
-- ============================================================

create or replace function forge_notify_work(
    p_kind text,
    p_queue_id uuid,
    p_story_id text
)
returns void
language plpgsql
as $$
begin
    if p_kind not in ('enqueued', 'armed', 'settled', 'claimed') then
        raise exception 'forge_notify_work: unknown kind % (a listener that has to guess is not being woken)', p_kind
            using errcode = '22023';
    end if;

    -- No length guard: `pg_notify` already refuses a payload over its own limit with its own error, and a second
    -- check here would restate that limit in a message that goes stale the day the limit moves.
    perform pg_notify(
        'forge_work_queue',
        jsonb_build_object(
            'kind', p_kind,
            'id', p_queue_id,
            'story_id', p_story_id,
            'at', to_char(now() at time zone 'utc', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        )::text);
end;
$$;

comment on function forge_notify_work(text, uuid, text) is
    'The one writer of the forge_work_queue doorbell payload: {kind, id, story_id, at}. Nulls mean the event has no single subject.';

-- ============================================================
-- ENQUEUE — 269's door, now with a kind on the bell.
-- ============================================================

create or replace function forge_enqueue_work(
    p_job_name text,
    p_story_id text,
    p_harness text,
    p_execution_target text,
    p_priority integer default 100
)
returns uuid
language plpgsql
as $$
declare
    v_id uuid;
begin
    if p_harness not in ('opencode', 'maestro') then
        raise exception 'invalid harness: %', p_harness;
    end if;

    insert into forge_work_queue (
        job_name,
        story_id,
        harness,
        execution_target,
        priority
    )
    values (
        p_job_name,
        p_story_id,
        p_harness,
        p_execution_target,
        p_priority
    )
    returning id into v_id;

    -- Doorbell only. The queue row remains the durable truth.
    perform forge_notify_work('enqueued', v_id, p_story_id);

    return v_id;
end;
$$;

-- ============================================================
-- ARM — 270's door, ringing once per row that took a slot.
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
            -- A row took a slot, so a story that was waiting is now dispatched. This is the event a waiting worker
            -- most wants: the door opened by itself and there is work behind it.
            perform forge_notify_work('armed', v_row.id, v_row.story_id);
        end if;
    end loop;

    return v_armed;
end;
$$;

-- ============================================================
-- SETTLE — 271's four branches, ringing once per row that left `Running`.
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
    if v_rows > 0 then
        -- A slot was freed. No single subject: the notification names none rather than picking one of the rows.
        perform forge_notify_work('settled', null, null);
    end if;

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
        -- `forge_fail_work` moved the row, so this is a door event: the slot it held is free.
        perform forge_notify_work('settled', v_row.id, v_row.story_id);
    end loop;

    -- 2b. The attempt ended badly and the board does not expect another one. The story's latest claim is terminal
    --     (`Error` / `Cancelled` — 263's settlement is its only writer), so no run is coming for this row, and the
    --     reason is the settlement's own words. Without this branch a refused `Done` (board still `In Progress`,
    --     story held) keeps its slot forever and the inlet closes on its own pacing: the PROD jam above.
    for v_row in
        select q.id, q.story_id, w.state as item_state, w.error_text
          from forge_work_queue q
          join storyboard_story s on s.id = q.story_id
          join lateral (
              select w2.state, w2.error_text
                from agent_work_item w2
               where w2.story_id = q.story_id
               order by w2.created_at desc, w2.id desc
               limit 1) w on true
         where q.state = 'Running'
           and w.state in ('Error', 'Cancelled')
           and s.status not in ('Planned', 'Ready', 'Complete')
           and not exists (select 1 from process_instances p
                            where p.subject_type = 'story' and p.subject_id = q.story_id
                              and p.status in ('active', 'running', 'reserved', 'suspended'))
    loop
        perform forge_fail_work(
            v_row.id,
            coalesce(
                nullif(btrim(coalesce(v_row.error_text, '')), ''),
                format('run for story %s ended %s', v_row.story_id, v_row.item_state)));
        v_settled := v_settled + 1;
        perform forge_notify_work('settled', v_row.id, v_row.story_id);
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
    if v_rows > 0 then
        -- Rows came back to the FIFO: work is claimable again, subject unnamed for the reason branch 1 gives.
        perform forge_notify_work('settled', null, null);
    end if;

    -- 4. Attempts spent: nothing will claim it again, so it must not keep reading as Pending.
    update forge_work_queue q
       set state = 'Error',
           last_error = coalesce(q.last_error, 'attempts exhausted'),
           completed_at = now(),
           updated_at = now()
     where q.state = 'Pending' and q.attempts >= q.max_attempts;
    get diagnostics v_rows = row_count;
    v_settled := v_settled + v_rows;
    if v_rows > 0 then
        perform forge_notify_work('settled', null, null);
    end if;

    return v_settled;
end;
$$;

commit;
