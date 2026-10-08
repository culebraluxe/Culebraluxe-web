-- 277_forge_agent_work_lease.sql
--
-- WHY: the queue's heartbeat was `updated_at = now()` with no owner. Any process could freshen any item,
-- `stale_agent_work` was the only fence, and a worker's death was invisible until the 15-minute sweep read
-- the row. Multi-worker scale-out needs a lease instead of a sweep: the claim names its owner and an
-- expiry, only the owner may beat it, and an expired lease is reclaimable on the next claim without waiting
-- for the sweep. Attempts already lived on the row (migration 028) but were invisible to the queue reader;
-- `ForgeQueueWorkRow` now surfaces them with the lease.
--
-- WHAT CHANGES:
--   1. `agent_work_item` gains `heartbeat_at`, `lease_expires_at` and `settlement_key`.
--   2. The two claim routines (migration 262) also open the lease and now treat an expired lease as
--      unheld: an item stuck `Claimed`/`Running` under a dead worker is claimable again. Eligibility by
--      policy, ordering and the one-active-per-story indexes are unchanged.
--   3. `forge_record_tool_artifact` (migration 267) takes a caller idempotency key; a retry with the same
--      key returns the FIRST row instead of writing a duplicate.
--   4. `forge_finish_agent_work_run` (migration 263) takes a caller idempotency key; a retry against an
--      already-settled item returns its stored settlement rather than settling again.
--
-- REVERSAL (277): drop the two columns and `settlement_key`, restore the 262/267/263 routine bodies without
-- the new parameters. Non-destructive: three added columns, one added nullable value, widened parameters.

begin;

alter table agent_work_item
    add column if not exists heartbeat_at timestamptz,
    add column if not exists lease_expires_at timestamptz,
    add column if not exists settlement_key text;

alter table forge_tool_artifact
    add column if not exists idempotency_key text;

-- Idempotency keys are unique where present: a retried submit is the same fact, not a second row.
create unique index if not exists agent_work_item_settlement_key_key
    on agent_work_item (settlement_key) where settlement_key is not null;

create unique index if not exists forge_tool_artifact_idempotency_key_key
    on forge_tool_artifact (idempotency_key) where idempotency_key is not null;

-- The specific claim: unchanged semantics, plus an open lease, plus expired-lease reclaim.
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
    -- its sibling slots are the parallel work it is part of. An item whose lease has expired still counts as
    -- open — the reclaim below re-marks THIS row, never a second writer over the story.
    if exists (
        select 1
          from agent_work_item a
         where a.state in ('Claimed', 'Running', 'Paused')
           and (v_group is null or a.parallel_group_id is null)
           and a.story_id = (select w.story_id from agent_work_item w where w.id = p_work_item_id)
           and a.id <> p_work_item_id
           -- A sibling whose lease died does not hold the story hostage: only a live one blocks.
           and (a.state = 'Paused' or a.lease_expires_at is null or a.lease_expires_at >= now())
    ) then
        return;
    end if;

    return query
    update agent_work_item w
       set state = 'Claimed', claimed_at = now(), claimed_by = p_worker_id,
           attempts = w.attempts + 1, updated_at = now(), heartbeat_at = now(),
           lease_expires_at = now() + interval '5 minutes'
     where w.id = p_work_item_id
       and (w.state = 'Ready'
            or (w.state in ('Claimed', 'Running')
                and w.lease_expires_at is not null
                and w.lease_expires_at < now()))
    returning w.*;
end;
$$;

-- THE CLAIM DOOR (275), with lease stamping and expired-lease reclaim. 275's brake, ceiling and ordering are
-- unchanged; the only new facts are that a fresh claim opens a lease and that an item stuck Claimed/Running under
-- a dead lease is claimable again — a dead worker is no longer a 15-minute hold.
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

    perform pg_advisory_xact_lock(9000212);

    select c.paused, c.updated_by, c.global_story_concurrency
      into v_paused, v_paused_by, v_ceiling
      from forge_runtime_control c
     where c.id = 1;

    if not found then
        raise exception 'forge_claim_story: forge_runtime_control row 1 is missing (no brake to read)'
            using errcode = 'P0001';
    end if;

    if v_paused then
        raise warning 'forge_claim_story: paused by % - no claim taken', coalesce(v_paused_by, 'unknown');
        return;
    end if;

    select count(distinct w.story_id) into v_running
      from agent_work_item w
     where w.state in ('Claimed', 'Running', 'Paused');

    v_slots := least(p_max, v_ceiling - v_running);
    if v_slots <= 0 then
        return;
    end if;

    -- 262's order, kept. Eligibility grows by exactly one row: an item stuck Claimed/Running whose lease has
    -- expired — the worker that held it is gone, the item is the queue's to hand out again.
    for v_candidate in
        select w.id
          from agent_work_item w
          join storyboard_story s on s.id = w.story_id
         where s.status = 'Ready'
           and w.execution_policy = 'Unattended OK'
           and (w.state = 'Ready'
                or (w.state in ('Claimed', 'Running')
                    and w.lease_expires_at is not null
                    and w.lease_expires_at < now()))
         order by w.priority desc, w.queued_at asc, w.id
           for update of w skip locked
         limit v_slots
    loop
        update agent_work_item w
           set state = 'Claimed', claimed_at = now(), claimed_by = p_worker_id,
               attempts = w.attempts + 1, updated_at = now(), heartbeat_at = now(),
               lease_expires_at = now() + interval '5 minutes'
         where w.id = v_candidate.id
           and (w.state = 'Ready'
                or (w.state in ('Claimed', 'Running')
                    and w.lease_expires_at is not null
                    and w.lease_expires_at < now()))
        returning w.* into v_claimed;

        if found then
            perform forge_notify_work('claimed', null, v_claimed.story_id);
            return next v_claimed;
        end if;
    end loop;

    return;
end;
$$;

comment on function forge_claim_story(text, integer) is
    'The claim door: refuses while forge_runtime_control.paused, honours global_story_concurrency, oldest-first; 277 adds lease_expires_at and expired-lease reclaim.';

create or replace function forge_claim_next_agent_work(p_worker_id text)
returns setof agent_work_item
language sql
as $$
    select * from forge_claim_story(p_worker_id, 1);
$$;

comment on function forge_claim_next_agent_work(text) is
    '262''s name, kept for existing callers: the mutex, the brake and the ceiling all live in forge_claim_story.';

-- The one terminal write, with a caller idempotency key: a retry with the same key against an already-settled
-- item returns its stored settlement instead of settling again. The settle logic itself is unchanged (263).
drop function if exists forge_finish_agent_work_run(uuid, text, text);

create function forge_finish_agent_work_run(p_work_item_id uuid, p_outcome text, p_error_text text, p_idempotency_key text default null)
returns table (item_state text, story_status text, reason text)
language plpgsql
as $$
declare
    v_board text;
    v_attempts integer;
    v_max_attempts integer;
    v_pair record;
    v_reason text;
    v_changed integer;
begin
    -- The idempotent-retry answer comes first: the item already settled under this key, return its row.
    if p_idempotency_key is not null then
        return query
        select w.state, s.status, w.error_text
          from agent_work_item w
          join storyboard_story s on s.id = w.story_id
         where w.id = p_work_item_id and w.settlement_key = p_idempotency_key
           and w.state in ('Done', 'Error', 'Cancelled');
        if found then
            return;
        end if;
    end if;

    select s.status, i.attempts, coalesce(i.max_attempts, 3)
      into v_board, v_attempts, v_max_attempts
      from agent_work_item i
      join storyboard_story s on s.id = i.story_id
     where i.id = p_work_item_id and i.state in ('Claimed', 'Running')
       for update of i;
    if not found then
        return;
    end if;

    select * into v_pair from forge_settlement_pair(p_outcome, v_board);
    if v_pair.item_state = 'Ready' and v_attempts >= v_max_attempts then
        select * into v_pair from forge_settlement_pair('Error', v_board);
    end if;

    v_reason := case
        when v_pair.reason is not null and p_error_text is not null then v_pair.reason || '; ' || p_error_text
        when v_pair.reason is not null then v_pair.reason
        else p_error_text
    end;
    -- A `Done` row carries no error text.
    if v_pair.item_state = 'Done' then
        v_reason := null;
    end if;

    -- A cleared row is back in the queue, so its claim and its settlement key are unset with it.
    if v_pair.item_state = 'Ready' then
        update agent_work_item w
           set state = 'Ready', error_text = v_reason, claimed_at = null, claimed_by = null, started_at = null,
               finished_at = null, updated_at = now(), settlement_key = null,
               heartbeat_at = null, lease_expires_at = null
         where w.id = p_work_item_id and w.state in ('Claimed', 'Running');
    else
        update agent_work_item w
           set state = v_pair.item_state, error_text = v_reason, finished_at = now(), updated_at = now(),
               settlement_key = p_idempotency_key
         where w.id = p_work_item_id and w.state in ('Claimed', 'Running');
    end if;
    get diagnostics v_changed = row_count;
    if v_changed = 0 then
        return;
    end if;

    if v_pair.story_status is not null then
        update storyboard_story s
           set status = v_pair.story_status, completed_at = null, updated_at = now()
         where s.id = (select w.story_id from agent_work_item w where w.id = p_work_item_id);
    end if;

    perform forge_close_story_run(p_work_item_id, forge_run_result_status_for(v_pair.item_state), v_reason);

    item_state := v_pair.item_state;
    story_status := v_pair.story_status;
    reason := v_pair.reason;
    return next;
end;
$$;

-- The artifact write, with a caller idempotency key: a retry with the same key returns the first row. The
-- ruling-vs-run polarity policy and everything else about the write is unchanged (267).
drop function if exists forge_record_tool_artifact(text, uuid, text, text, text, text, jsonb, text);

create function forge_record_tool_artifact(
    p_story_id text,
    p_story_run_id uuid,
    p_tool text,
    p_kind text,
    p_verdict text,
    p_summary text,
    p_detail jsonb,
    p_sha text,
    p_idempotency_key text default null
)
returns table (
    id text, story_id text, story_run_id text, tool text, kind text,
    verdict text, summary text, sha text, created_at text
)
language plpgsql
as $$
#variable_conflict use_column
declare
    v_ruling text;
begin
    -- The idempotent-retry answer comes first.
    if p_idempotency_key is not null then
        return query
        select a.id::text, a.story_id, a.story_run_id::text, a.tool, a.kind,
               a.verdict, a.summary, a.sha, a.created_at::text
          from forge_tool_artifact a
         where a.idempotency_key = p_idempotency_key;
        if found then
            return;
        end if;
    end if;

    if p_story_run_id is not null then
        select r.result_status into v_ruling from storyboard_story_run r where r.id = p_story_run_id;
    end if;

    begin
        return query
        insert into forge_tool_artifact as a (story_id, story_run_id, tool, kind, verdict, summary, detail, sha, idempotency_key)
        values (p_story_id, p_story_run_id, p_tool, p_kind,
                forge_artifact_verdict_for_run(p_kind, v_ruling, p_verdict), p_summary, p_detail, p_sha, p_idempotency_key)
        returning a.id::text, a.story_id::text, a.story_run_id::text, a.tool::text, a.kind::text,
                  a.verdict::text, a.summary::text, a.sha::text, a.created_at::text;
    exception when unique_violation then
        -- A concurrent twin with the same key won the race: its row is the answer, not an error.
        return query
        select a.id::text, a.story_id, a.story_run_id::text, a.tool, a.kind,
               a.verdict, a.summary, a.sha, a.created_at::text
          from forge_tool_artifact a
         where a.idempotency_key = p_idempotency_key;
    end;
end;
$$;

comment on function forge_record_tool_artifact(text, uuid, text, text, text, text, jsonb, text, text) is
    'The one write of forge_tool_artifact. A caller idempotency key makes a retry the same fact, not a second row.';

-- The settle's old 3-arg signature is replaced; the forge_worker grant must name the new one.
do $$
begin
    if exists (select 1 from pg_roles where rolname = 'forge_worker') then
        grant execute on function forge_finish_agent_work_run(uuid, text, text, text),
                                forge_record_tool_artifact(text, uuid, text, text, text, text, jsonb, text, text)
          to forge_worker;
    end if;
end $$;

commit;
