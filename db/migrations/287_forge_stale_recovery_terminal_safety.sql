-- Preserve terminal work and the human-hold explanation during fenced stale recovery.
begin;

create or replace function forge_recover_stale_work(
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
    if v_item.state not in ('Claimed','Running','Paused') then
        return 'terminal_preserved';
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
           error_text=case when v_story.status='Hold' then 'stale claim on a story a human holds'
                           when v_target_state='Done' then null
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

commit;
