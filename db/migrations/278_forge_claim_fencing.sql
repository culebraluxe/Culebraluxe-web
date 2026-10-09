-- 278_forge_claim_fencing.sql
--
-- WHY: a claim named its owner but not its AUTHORITY. `claimed_by` is text, and lifetime authority was
-- (owner text, state) — so a worker that lost its claim to a successor of the same name, or a child process
-- still running behind a requeued item, could still begin a run, freshen the replacement's lease, settle the
-- replacement's work, or close a run somebody else owns. `agent_work_item.claim_generation` is added and bumped
-- by the claim itself, so (id, claim_generation) names exactly ONE execution; every fenced transition below
-- validates owner AND generation AND state atomically under a row lock, and answers with a typed result instead
-- of a bare row-or-nothing.
--
-- WHAT CHANGES:
--   1. `agent_work_item` gains `claim_generation` (NOT NULL, default 0). 0 is the pre-278 authority: an item
--      claimed by a worker that does not know about generations still owns its claim at generation 0, so a
--      drain/cutover does not orphan in-flight work (§10.8 of the work order).
--   2. Both claim routines (262's specific claim, 275's door) bump the generation in the same statement that
--      names the owner and opens the lease. Reclaiming an expired lease bumps it too: a reclaim is a NEW
--      authority, never the old one resumed.
--   3. `forge_begin_agent_work_run`, `forge_heartbeat_agent_work` and `forge_finish_agent_work_run` require
--      (owner, generation) and refuse anything else. The settle returns a typed result row
--      (settled | duplicate | released | refused_ownership | conflict | not_found) rather than an empty set, so
--      "you lost the claim" and "no such item" are not the same answer.
--   4. Settlement idempotency is derived BY THE ROUTINE from (work item, generation, outcome). A caller cannot
--      invent a settlement key, a retry by the same execution is a duplicate, and the same execution changing
--      its outcome under the same identity is a conflict, not a second verdict.
--
-- REVERSAL (278): drop the three fenced routines and restore the 264/277 signatures without the owner/generation
-- parameters and without the typed result; drop the column. The column is dropped LAST: a rollback that removes
-- it while a new binary is running leaves that binary unable to fence.
--
-- NOT in this migration: the recovery writers (`forge_requeue_stale_work`, `forge_hold_stale_work`,
-- `forge_recover_stale_engine_claim`) are fenced by migration 279, which is Slice 4 of the same work order.

begin;

alter table agent_work_item
    add column if not exists claim_generation bigint not null default 0;

comment on column agent_work_item.claim_generation is
    'Claim authority: bumped by every successful claim/reclaim. (`id`, `claim_generation`) names one execution, so reusing a worker name never reuses authority. 0 = the pre-278 authority of a claim nobody fenced.';

-- 275''s routine also documented here, so `comment on function` travels with the redefinition below.

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
               attempts = w.attempts + 1, claim_generation = w.claim_generation + 1, updated_at = now(), heartbeat_at = now(),
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
           attempts = w.attempts + 1, claim_generation = w.claim_generation + 1, updated_at = now(), heartbeat_at = now(),
           lease_expires_at = now() + interval '5 minutes'
     where w.id = p_work_item_id
       and (w.state = 'Ready'
            or (w.state in ('Claimed', 'Running')
                and w.lease_expires_at is not null
                and w.lease_expires_at < now()))
    returning w.*;
end;
$$;
-- ============================================================================================
-- THE FENCED TRANSITIONS. Owner text alone was never authority: `claimed_by` is a name, and a
-- name is reused (AGENT_WORKER_ID is exported by hand and by host). Every routine below takes
-- (owner, generation) and validates them against the row under `for update`.
-- ============================================================================================

-- Open the run, fenced. `Claimed → Running` for THIS execution only: a successor that reclaimed the
-- item (new generation) or a peer that never held it gets no row and no run, and the story run row is
-- not opened for them either. Migration 264's snapshot, guard and envelope are unchanged.
drop function if exists forge_begin_agent_work_run(uuid, text);

create or replace function forge_begin_agent_work_run(
    p_work_item_id uuid,
    p_execution_environment text,
    p_claim_owner text,
    p_claim_generation bigint
)
returns table (execution_policy text, story_run_id text, model_policy text, launch_intent text, claim_generation bigint)
language plpgsql
as $$
declare
    v_run storyboard_story_run.id%type;
begin
    -- Locked before it is moved: the story cannot change under the run this opens, and a second begin on the same
    -- row is a no-op rather than a second run. The fence is part of the lock predicate, so a superseded execution
    -- does not even queue behind the successor's transaction.
    perform 1
       from agent_work_item w
      where w.id = p_work_item_id and w.state = 'Claimed'
        and w.claimed_by = p_claim_owner
        and w.claim_generation = p_claim_generation
        for update;
    if not found then
        return;
    end if;

    insert into storyboard_story_run
        (story_id, started_at, execution_environment, run_type,
         goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
         context_refs_snapshot, acceptance_criteria_snapshot, postconditions_snapshot,
         dependencies_snapshot, scope_snapshot, operating_surface_snapshot,
         test_mode_snapshot, assay_commands_snapshot, packet_sha_snapshot)
    select s.id, now(), p_execution_environment,
           coalesce(nullif(trim(i.role), ''), nullif(trim(i.kind), ''), 'dispatch'),
           nullif(trim(s.goal), ''), nullif(trim(s.preconditions), ''),
           nullif(trim(s.architect_brief), ''), nullif(trim(s.context_refs), ''),
           nullif(trim(s.acceptance_criteria), ''), nullif(trim(s.postconditions), ''),
           nullif(trim(s.dependencies), ''), nullif(trim(s.scope), ''),
           nullif(trim(s.operating_surface), ''), nullif(trim(s.test_mode), ''),
           nullif(trim(s.assay_commands), ''), nullif(trim(s.packet_sha), '')
      from agent_work_item i
      join storyboard_story s on s.id = i.story_id
     where i.id = p_work_item_id
    returning storyboard_story_run.id into v_run;

    return query
    update agent_work_item w
       set state = 'Running', started_at = coalesce(w.started_at, now()),
           story_run_id = v_run, updated_at = now()
     where w.id = p_work_item_id and w.state = 'Claimed'
       and w.claimed_by = p_claim_owner
       and w.claim_generation = p_claim_generation
    returning w.execution_policy::text, v_run::text, w.model_policy::text, w.launch_intent::text, w.claim_generation;
end;
$$;

comment on function forge_begin_agent_work_run(uuid, text, text, bigint) is
    'The fenced begin: Claimed → Running for the one execution that owns (owner, generation). No row = not ours.';

-- The heartbeat was a Rust `update ... where claimed_by = $2` (db/src/forge_engine.rs). Owner-fenced but not
-- GENERATION-fenced, so a dead execution's beat landing after a reclaim refreshed the SUCCESSOR's lease and kept
-- a live run invisible to stale recovery. Renewing the lease is the one write that decides whether a claim is
-- alive, so it is a fenced transition like the others: owner, generation and a claimable state, or false.
create or replace function forge_heartbeat_agent_work(
    p_work_item_id uuid,
    p_claim_owner text,
    p_claim_generation bigint,
    p_lease_seconds double precision
)
returns boolean
language plpgsql
as $$
declare
    v_updated integer;
begin
    update agent_work_item w
       set updated_at = now(), heartbeat_at = now(),
           lease_expires_at = now() + make_interval(secs => p_lease_seconds)
     where w.id = p_work_item_id
       and w.state in ('Claimed', 'Running')
       and w.claimed_by = p_claim_owner
       and w.claim_generation = p_claim_generation;
    get diagnostics v_updated = row_count;
    return v_updated > 0;
end;
$$;

comment on function forge_heartbeat_agent_work(uuid, text, bigint, double precision) is
    'Renew the lease for the one execution that owns (owner, generation). False = no longer claimable.';


-- The one terminal write, fenced and typed.
--
-- Two facts the old shape could not express, and both were load-bearing:
--   * `settlement_key` was a caller-supplied parameter, so idempotency was a promise from the caller. It is now
--     derived HERE from (work item, generation, outcome): a retry by the same execution is a duplicate, the same
--     execution changing its outcome under the same authority is a conflict, and no caller can invent an identity
--     that makes a second write look like a first.
--   * The answer was an empty row set for every kind of "no" — already settled, not mine, no such item. The caller
--     printed "already had a verdict" for all three, which is how a superseded execution learned to look like a
--     settled one. The result is now named: settled | duplicate | released | conflict | refused_ownership | not_found.
drop function if exists forge_finish_agent_work_run(uuid, text, text, text);

create or replace function forge_finish_agent_work_run(
    p_work_item_id uuid,
    p_outcome text,
    p_error_text text,
    p_claim_owner text,
    p_claim_generation bigint
)
returns table (result text, item_state text, story_status text, reason text)
language plpgsql
as $$
declare
    v_board text;
    v_attempts integer;
    v_max_attempts integer;
    v_state text;
    v_owner text;
    v_generation bigint;
    v_stored_key text;
    v_stored_error text;
    v_pair record;
    v_reason text;
    v_changed integer;
    v_generation_text text := coalesce(p_claim_generation, 0)::text;
    v_prefix text;
    v_key text;
begin
    v_prefix := p_work_item_id::text || ':' || v_generation_text || ':';
    v_key := v_prefix || coalesce(p_outcome, '');

    -- ONE lock, one read, one decision. The board comes back with the item so the pair is chosen from the state
    -- the transaction actually holds.
    select s.status, i.attempts, coalesce(i.max_attempts, 3),
           i.state, i.claimed_by, i.claim_generation, i.settlement_key, i.error_text
      into v_board, v_attempts, v_max_attempts,
           v_state, v_owner, v_generation, v_stored_key, v_stored_error
      from agent_work_item i
      join storyboard_story s on s.id = i.story_id
     where i.id = p_work_item_id
       for update of i;
    if not found then
        result := 'not_found';
        return next;
        return;
    end if;

    -- Already terminal: this write cannot happen, so answer from the row. A retry of the SAME execution's settle
    -- is a duplicate; a different outcome under the same authority is a conflict; anything else is somebody else's
    -- verdict and is refused.
    if v_state in ('Done', 'Error', 'Cancelled') then
        if v_stored_key = v_key then
            result := 'duplicate';
        elsif v_stored_key is not null and left(v_stored_key, length(v_prefix)) = v_prefix then
            result := 'conflict';
        else
            result := 'refused_ownership';
        end if;
        item_state := v_state;
        story_status := v_board;
        reason := v_stored_error;
        return next;
        return;
    end if;

    -- Not claimable. A row THIS generation released is the queue's again and the caller's settle was already
    -- applied (an `Abandoned` clears the owner but keeps the generation, so the pair cannot be mistaken for a
    -- fresh row). Generation 0 is the pre-278 authority and never claims a release it cannot prove.
    if v_state not in ('Claimed', 'Running') then
        if v_state = 'Ready' and v_generation = p_claim_generation and v_generation > 0 then
            result := 'released';
        else
            result := 'refused_ownership';
        end if;
        item_state := v_state;
        story_status := v_board;
        reason := v_stored_error;
        return next;
        return;
    end if;

    -- The fence itself. A successor that reclaimed the item owns it now; this execution may not write over it.
    if v_owner is distinct from p_claim_owner or v_generation <> p_claim_generation then
        result := 'refused_ownership';
        item_state := v_state;
        story_status := v_board;
        reason := v_stored_error;
        return next;
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

    -- A cleared row is back in the queue, so its claim and its settlement key are unset with it; the generation
    -- stays, because it names the execution that held it and the next claim bumps it.
    if v_pair.item_state = 'Ready' then
        update agent_work_item w
           set state = 'Ready', error_text = v_reason, claimed_at = null, claimed_by = null, started_at = null,
               finished_at = null, updated_at = now(), settlement_key = null,
               heartbeat_at = null, lease_expires_at = null
         where w.id = p_work_item_id and w.state in ('Claimed', 'Running')
           and w.claimed_by = p_claim_owner and w.claim_generation = p_claim_generation;
    else
        update agent_work_item w
           set state = v_pair.item_state, error_text = v_reason, finished_at = now(), updated_at = now(),
               settlement_key = v_key
         where w.id = p_work_item_id and w.state in ('Claimed', 'Running')
           and w.claimed_by = p_claim_owner and w.claim_generation = p_claim_generation;
    end if;
    get diagnostics v_changed = row_count;
    if v_changed = 0 then
        -- The row was locked and read one statement ago, so this is the database disagreeing with itself rather
        -- than a race. Reporting a settle that wrote nothing is the one thing this module must not do.
        result := 'refused_ownership';
        item_state := v_state;
        story_status := v_board;
        reason := 'the work item moved under the settle transaction';
        return next;
        return;
    end if;

    if v_pair.story_status is not null then
        update storyboard_story s
           set status = v_pair.story_status, completed_at = null, updated_at = now()
         where s.id = (select w.story_id from agent_work_item w where w.id = p_work_item_id);
    end if;

    perform forge_close_story_run(p_work_item_id, forge_run_result_status_for(v_pair.item_state), v_reason);

    result := 'settled';
    item_state := v_pair.item_state;
    story_status := v_pair.story_status;
    reason := v_pair.reason;
    return next;
end;
$$;

comment on function forge_finish_agent_work_run(uuid, text, text, text, bigint) is
    'The fenced settlement: one terminal write for the execution that owns (owner, generation), with a settlement key derived from (item, generation, outcome) and a typed result.';

-- The old unfenced signatures are gone, so the grant must name the new ones.
do $$
begin
    if exists (select 1 from pg_roles where rolname = 'forge_worker') then
        grant execute on function forge_begin_agent_work_run(uuid, text, text, bigint),
                                forge_heartbeat_agent_work(uuid, text, bigint, double precision),
                                forge_finish_agent_work_run(uuid, text, text, text, bigint)
          to forge_worker;
    end if;
end $$;


commit;
