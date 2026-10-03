-- 266_forge_stale_recovery.sql
--
-- WHY: the three stale-recovery writes were transactions choreographed from Rust — `ForgeControlDao::
-- hold_stale_work` and `requeue_stale_work` (rust/core/db/src/forge_control.rs) and the per-claim transaction of
-- `ForgeResetDao::recover_stale_engine_claims` (rust/core/db/src/forge_reset.rs). Each is one atomic database
-- write, so each lives here now and the DAO binds parameters. Translated as-is — the fifth stored-routine slice
-- after 262–265. The POLICY that chooses between hold and requeue (role, attempts) and the run interrupt stay
-- with the worker that owns the sweep; the candidate list stays a plain read; and the engine-claim sweep keeps
-- ONE TRANSACTION PER CLAIM (its Rust loop calls `forge_recover_stale_engine_claim` once per candidate), so a
-- failure in one never rolls back another.

begin;

-- Hold a stale claim the worker will not retry: the item records `Error`, the story goes to `Hold` with it.
create or replace function forge_hold_stale_work(p_work_item_id uuid, p_story_id text, p_reason text)
returns void
language plpgsql
as $$
begin
    update agent_work_item w
       set state = 'Error', error_text = p_reason, finished_at = now(), updated_at = now()
     where w.id = p_work_item_id and w.state in ('Claimed', 'Running', 'Paused');
    update storyboard_story s
       set status = 'Hold', completed_at = null, updated_at = now()
     where s.id = p_story_id;
end;
$$;

-- Give a stale claim back to the queue — but only to a story that can still be worked. A requeue moves both rows
-- or neither, and the board is read first (2026-09-29: requeueing whatever the board said ran stories twice):
--   * `Complete`: the work landed; the stale row is bookkeeping and settles `Done`;
--   * `Hold`: a human gate owns the story; the claim is ended, the board is not reopened;
--   * otherwise: the run is retried and the story moves back to `Ready` with it.
create or replace function forge_requeue_stale_work(p_work_item_id uuid, p_story_id text)
returns void
language plpgsql
as $$
declare
    v_board text;
begin
    select s.status
      into v_board
      from agent_work_item i
      join storyboard_story s on s.id = i.story_id
     where i.id = p_work_item_id and i.state in ('Claimed', 'Running', 'Paused')
       for update of i;
    if not found then
        return;
    end if;

    if v_board = 'Complete' then
        update agent_work_item w
           set state = 'Done', error_text = null, finished_at = now(), updated_at = now()
         where w.id = p_work_item_id and w.state in ('Claimed', 'Running', 'Paused');
    elsif v_board = 'Hold' then
        update agent_work_item w
           set state = 'Error', error_text = 'stale claim on a story a human holds',
               finished_at = now(), updated_at = now()
         where w.id = p_work_item_id and w.state in ('Claimed', 'Running', 'Paused');
    else
        update agent_work_item w
           set state = 'Ready', queued_at = now(), claimed_at = null, claimed_by = null,
               started_at = null, finished_at = null, error_text = null, runtime_adapter = null,
               external_run_id = null, updated_at = now()
         where w.id = p_work_item_id and w.state in ('Claimed', 'Running', 'Paused');
        update storyboard_story s
           set status = 'Ready', completed_at = null, updated_at = now()
         where s.id = p_story_id;
    end if;
end;
$$;

-- Recover ONE stale engine claim. True = recovered (execution interrupted, item released); false = skipped, with
-- nothing written. Lock order is the engine's: the process instance first, then the execution row, re-read and
-- decided again under lock. Freshness is the database's (`heartbeat_at` against the cutoff). The CAS keeps its row
-- count as the load-bearing guard: a sweep that reports a miss while it wrote the row cannot be trusted.
create or replace function forge_recover_stale_engine_claim(
    p_task_id uuid,
    p_process_instance_id uuid,
    p_work_item_id uuid,
    p_stale_minutes integer
)
returns boolean
language plpgsql
as $$
declare
    v_cutoff interval := (p_stale_minutes::text || ' minutes')::interval;
    v_status text;
    v_fresh boolean;
    v_updated integer;
begin
    perform 1 from process_instances p where p.id = p_process_instance_id for update;

    select e.status, (e.heartbeat_at > now() - v_cutoff)
      into v_status, v_fresh
      from forge_engine_task_execution e
     where e.task_id = p_task_id
       for update;
    if not found or v_status not in ('claimed', 'running') or v_fresh is true then
        return false;
    end if;

    update forge_engine_task_execution e
       set status = 'interrupted', last_error = 'stale claim recovered', updated_at = now()
     where e.task_id = p_task_id
       and e.status in ('claimed', 'running')
       and e.heartbeat_at <= now() - v_cutoff;
    get diagnostics v_updated = row_count;
    if v_updated = 0 then
        return false;
    end if;

    update agent_work_item w
       set state = 'Ready', claimed_by = null,
           error_text = 'stale claim recovered; awaiting fresh attempt', updated_at = now()
     where w.id = p_work_item_id and w.state in ('Claimed', 'Running');
    return true;
end;
$$;

commit;
