-- FORGE-B1 Slice 4: stale recovery is authorized by the claim snapshot, not by an item id.
-- Lock order follows forge_finish_agent_work_run: item, story, then run/execution history.
begin;

-- OUT columns are part of a PostgreSQL function's type; replace the old read-only candidate projection.
drop function forge_reapable_claims(integer);
create function forge_reapable_claims(p_stale_after_minutes integer)
returns table (
    id uuid, story_id text, role text, attempts integer, max_attempts integer,
    story_run_id uuid, updated_at timestamptz, why text,
    claimed_by text, claim_generation bigint, state text,
    heartbeat_at timestamptz, lease_expires_at timestamptz
)
language sql stable as $$
    select w.id, w.story_id, w.role, w.attempts, coalesce(w.max_attempts, 3),
           w.story_run_id, w.updated_at,
           case when coalesce(h.stale, false)
                then format('worker %s last beat %s seconds ago', w.claimed_by, h.age_seconds)
                else format('claim untouched for %s minutes (no worker heartbeat for this claim)',
                            greatest(coalesce(p_stale_after_minutes, 0), 0)) end,
           w.claimed_by, w.claim_generation, w.state, w.heartbeat_at, w.lease_expires_at
      from agent_work_item w
      left join forge_worker_health h on h.worker_id = w.claimed_by
     where w.state in ('Claimed', 'Running', 'Paused')
       and (w.updated_at < now() - (greatest(coalesce(p_stale_after_minutes, 0), 0)::text || ' minutes')::interval
            or (w.lease_expires_at is not null and w.lease_expires_at < now())
            or coalesce(h.stale, false))
     order by w.updated_at asc;
$$;

alter table forge_engine_task_execution add column if not exists claim_generation bigint;
comment on column forge_engine_task_execution.claim_generation is
    'Claim generation that authorized this engine execution. NULL legacy rows are not eligible for automatic stale recovery.';

create function forge_bind_engine_execution_generation()
returns trigger language plpgsql as $$
declare v_generation bigint;
begin
    select i.claim_generation into v_generation from agent_work_item i
     where i.id=new.work_item_id and i.claimed_by=new.worker_id
       and i.state in ('Claimed','Running') for share;
    if not found then
        raise exception 'engine execution has no matching active work-item owner'
            using errcode='23514';
    end if;
    if new.claim_generation is not null and new.claim_generation<>v_generation then
        raise exception 'engine execution claim generation does not match work item'
            using errcode='23514';
    end if;
    new.claim_generation := v_generation;
    return new;
end;
$$;

create trigger forge_engine_execution_generation_before_insert
before insert on forge_engine_task_execution
for each row execute function forge_bind_engine_execution_generation();

drop function forge_hold_stale_work(uuid, text, text);
drop function forge_requeue_stale_work(uuid, text);
drop function forge_recover_stale_engine_claim(uuid, uuid, uuid, integer);

create function forge_recover_stale_work(
    p_work_item_id uuid,
    p_expected_story_id text,
    p_expected_owner text,
    p_expected_generation bigint,
    p_expected_state text,
    p_observed_updated_at timestamptz,
    p_observed_heartbeat_at timestamptz,
    p_observed_lease_expires_at timestamptz,
    p_stale_minutes integer,
    p_reason text,
    p_failure_code text,
    p_task_id uuid default null,
    p_expected_execution_status text default null,
    p_observed_execution_heartbeat timestamptz default null
)
returns text
language plpgsql
as $$
declare
    v_item agent_work_item%rowtype;
    v_story storyboard_story%rowtype;
    v_execution forge_engine_task_execution%rowtype;
    v_changed integer;
    v_stale boolean;
    v_target_state text;
    v_story_target text;
    v_result text;
begin
    select * into v_item from agent_work_item where id=p_work_item_id for update;
    if not found then return 'ownership_changed'; end if;
    if v_item.story_id is distinct from p_expected_story_id then return 'conflict'; end if;
    if v_item.claim_generation is distinct from p_expected_generation
       or v_item.claimed_by is distinct from p_expected_owner
       or v_item.state is distinct from p_expected_state then
        return 'ownership_changed';
    end if;
    if v_item.updated_at is distinct from p_observed_updated_at
       or v_item.heartbeat_at is distinct from p_observed_heartbeat_at
       or v_item.lease_expires_at is distinct from p_observed_lease_expires_at then
        return 'no_longer_stale';
    end if;

    select * into v_story from storyboard_story where id=v_item.story_id for update;
    if not found then return 'conflict'; end if;

    if p_task_id is not null then
        select * into v_execution from forge_engine_task_execution
         where task_id=p_task_id for update;
        if not found or v_execution.work_item_id <> v_item.id
           or v_execution.claim_generation is distinct from p_expected_generation
           or v_execution.worker_id is distinct from p_expected_owner then
            return 'ownership_changed';
        end if;
        if v_execution.status is distinct from p_expected_execution_status
           or v_execution.heartbeat_at is distinct from p_observed_execution_heartbeat
           or v_execution.status not in ('claimed','running') then
            return 'no_longer_stale';
        end if;
        if v_execution.heartbeat_at > now() - make_interval(mins => greatest(p_stale_minutes, 1)) then
            return 'no_longer_stale';
        end if;
    else
        -- The agent-work sweep can also encounter an engine-owned run. Fence and lock that exact execution before
        -- changing the item; a fresh engine heartbeat vetoes recovery even if another timestamp looks old.
        select * into v_execution from forge_engine_task_execution
         where work_item_id=v_item.id and claim_generation=p_expected_generation
           and worker_id=p_expected_owner and status in ('claimed','running')
         for update;
        if found and v_execution.heartbeat_at > now() - make_interval(mins => greatest(p_stale_minutes, 1)) then
            return 'no_longer_stale';
        end if;
    end if;

    select (v_item.updated_at <= now() - make_interval(mins => greatest(p_stale_minutes, 1))
            or coalesce(h.stale, false)
            or (v_item.lease_expires_at is not null and v_item.lease_expires_at < now()))
      into v_stale
      from (select 1) seed left join forge_worker_health h on h.worker_id=v_item.claimed_by;
    if not coalesce(v_stale, false) then return 'no_longer_stale'; end if;

    if v_story.status = 'Complete' then
        v_target_state := 'Done'; v_story_target := null; v_result := 'terminal_preserved';
    elsif v_story.status = 'Hold' then
        v_target_state := 'Error'; v_story_target := null; v_result := 'terminal_preserved';
    elsif lower(coalesce(v_item.role,'')) in ('reviewer','verifier')
       or v_item.attempts >= coalesce(v_item.max_attempts,3) then
        v_target_state := 'Error'; v_story_target := 'Hold'; v_result := 'recovered';
    else
        v_target_state := 'Ready'; v_story_target := 'Ready'; v_result := 'recovered';
    end if;

    update agent_work_item w
       set state=v_target_state,
           claimed_by=case when v_target_state='Ready' then null else w.claimed_by end,
           claimed_at=case when v_target_state='Ready' then null else w.claimed_at end,
           started_at=case when v_target_state='Ready' then null else w.started_at end,
           finished_at=case when v_target_state='Ready' then null else now() end,
           queued_at=case when v_target_state='Ready' then now() else w.queued_at end,
           error_text=case when v_target_state='Done' then null
                           when v_target_state='Ready' then null else p_reason end,
           runtime_adapter=case when v_target_state='Ready' then null else w.runtime_adapter end,
           external_run_id=case when v_target_state='Ready' then null else w.external_run_id end,
           lease_expires_at=case when v_target_state='Ready' then null else w.lease_expires_at end,
           updated_at=now()
     where w.id=p_work_item_id and w.story_id=v_item.story_id
       and w.claimed_by is not distinct from p_expected_owner
       and w.claim_generation=p_expected_generation and w.state=p_expected_state
       and w.updated_at is not distinct from p_observed_updated_at
       and w.heartbeat_at is not distinct from p_observed_heartbeat_at
       and w.lease_expires_at is not distinct from p_observed_lease_expires_at;
    get diagnostics v_changed = row_count;
    if v_changed <> 1 then return 'conflict'; end if;

    -- Board writes happen only after the guarded item transition. Complete's completed_at is never touched.
    if v_story_target is not null then
        update storyboard_story set status=v_story_target, completed_at=null, updated_at=now()
         where id=v_item.story_id and status not in ('Complete','Hold');
    end if;
    if v_item.story_run_id is not null and v_story.status <> 'Complete' then
        update storyboard_story_run
           set ended_at=now(), result_status='Interrupted', failure_code=p_failure_code,
               notes=case when notes is null or notes='' then p_reason else notes || E'\\n' || p_reason end,
               updated_at=now()
         where id=v_item.story_run_id and ended_at is null;
    end if;
    if v_execution.task_id is not null then
        update forge_engine_task_execution
           set status=case when v_target_state='Done' then 'completed' else 'interrupted' end,
               completed_at=case when v_target_state='Done' then coalesce(completed_at,now()) else completed_at end,
               last_error=case when v_target_state='Done' then null else p_reason end, updated_at=now()
         where task_id=v_execution.task_id and work_item_id=p_work_item_id
           and claim_generation=p_expected_generation and worker_id=p_expected_owner
           and status=v_execution.status and heartbeat_at=v_execution.heartbeat_at;
        get diagnostics v_changed = row_count;
        if v_changed <> 1 then raise exception 'engine execution changed after stale recovery lock'; end if;
    end if;
    return v_result;
end;
$$;

comment on function forge_recover_stale_work(uuid,text,text,bigint,text,timestamptz,timestamptz,timestamptz,integer,text,text,uuid,text,timestamptz) is
    'CAS-fenced stale recovery. Locks work item then story, derives story identity from the item, and changes the board only after the item CAS succeeds.';

create function forge_cancel_stale_open_work(
    p_work_item_id uuid, p_expected_story_id text, p_expected_owner text,
    p_expected_generation bigint, p_expected_state text,
    p_observed_updated_at timestamptz, p_observed_heartbeat_at timestamptz,
    p_observed_lease_expires_at timestamptz, p_stale_minutes integer
)
returns text language plpgsql as $$
declare
    v_item agent_work_item%rowtype;
    v_story storyboard_story%rowtype;
    v_execution forge_engine_task_execution%rowtype;
    v_changed integer;
begin
    select * into v_item from agent_work_item where id=p_work_item_id for update;
    if not found then return 'ownership_changed'; end if;
    if v_item.story_id is distinct from p_expected_story_id then return 'conflict'; end if;
    if v_item.claimed_by is distinct from p_expected_owner
       or v_item.claim_generation is distinct from p_expected_generation
       or v_item.state is distinct from p_expected_state then return 'ownership_changed'; end if;
    if v_item.updated_at is distinct from p_observed_updated_at
       or v_item.heartbeat_at is distinct from p_observed_heartbeat_at
       or v_item.lease_expires_at is distinct from p_observed_lease_expires_at then
        return 'no_longer_stale';
    end if;
    select * into v_story from storyboard_story where id=v_item.story_id for update;
    if not found then return 'conflict'; end if;
    if v_story.status='Complete' then return 'terminal_preserved'; end if;
    select * into v_execution from forge_engine_task_execution
     where work_item_id=v_item.id and claim_generation=p_expected_generation
       and worker_id=p_expected_owner and status in ('claimed','running')
     for update;
    if found and v_execution.heartbeat_at > now()-make_interval(mins => greatest(p_stale_minutes,1)) then
        return 'no_longer_stale';
    end if;
    if not (coalesce(v_item.updated_at,v_item.created_at) <= now()-make_interval(mins => greatest(p_stale_minutes,1))
            or (v_item.lease_expires_at is not null and v_item.lease_expires_at < now())) then
        return 'no_longer_stale';
    end if;
    update agent_work_item set state='Cancelled', claimed_by=null, started_at=null,
           finished_at=now(), updated_at=now()
     where id=p_work_item_id and story_id=v_item.story_id
       and claimed_by is not distinct from p_expected_owner
       and claim_generation=p_expected_generation and state=p_expected_state
       and updated_at is not distinct from p_observed_updated_at
       and heartbeat_at is not distinct from p_observed_heartbeat_at
       and lease_expires_at is not distinct from p_observed_lease_expires_at;
    get diagnostics v_changed=row_count;
    if v_changed<>1 then return 'conflict'; end if;
    if v_execution.task_id is not null then
        update forge_engine_task_execution
           set status='interrupted', last_error='clean sweep', updated_at=now()
         where task_id=v_execution.task_id and work_item_id=p_work_item_id
           and claim_generation=p_expected_generation and worker_id=p_expected_owner
           and status=v_execution.status and heartbeat_at=v_execution.heartbeat_at;
        get diagnostics v_changed=row_count;
        if v_changed<>1 then raise exception 'engine execution changed after clean recovery lock'; end if;
    end if;
    update storyboard_story set status='Hold', completed_at=null, updated_at=now()
     where id=v_item.story_id and status in ('Ready','In Progress');
    return 'recovered';
end;
$$;

commit;
