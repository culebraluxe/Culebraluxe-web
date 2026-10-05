-- 269_forge_work_queue.sql
--
-- WHY: one durable FIFO work queue in Neon, so a batch of jobs (600 of them, say) can be enqueued as rows and
-- claimed one at a time by whichever worker asks next — instead of a job being held by a process, or re-typed
-- by hand per run.
--
-- WHAT IT IS: `forge_work_queue` (the row is the durable truth) plus five stored routines around it:
--
--   forge_enqueue_work(job_name, story_id, harness, execution_target, priority) -> uuid   the only write door
--   forge_claim_next_work(worker_id)                -> setof forge_work_queue             FIFO claim, SKIP LOCKED
--   forge_complete_work(id, commit_sha)             -> void                               settle, with the sha
--   forge_retry_work(id, error, delay_seconds)      -> void                               back to Pending, or Error at the cap
--   forge_fail_work(id, error)                      -> void                               terminal Error
--
-- CLAIM ORDER is `priority asc, created_at asc, id asc`, and the partial index below is that same key, so the
-- claim is an index-order read of the Pending rows and never a sort. `for update skip locked` inside the CTE is
-- what makes it safe for N concurrent workers: two workers cannot take one row, and a worker never blocks behind
-- another worker's in-flight claim.
--
-- THE PG_NOTIFY IS A DOORBELL, NOT THE QUEUE. `forge_enqueue_work` fires `forge_work_queue` with the new id so a
-- listener can wake immediately; nothing is answered from the notification, and a worker that misses it still
-- finds the row on its next poll. Deleting the notify changes only latency.
--
-- WHAT THIS MIGRATION DOES NOT CHANGE: Forge, the workflow engine, the model harnesses and Maestro are untouched.
-- No existing table, column, routine or ledger row is read or written here. `execution_target` is carried as text
-- exactly as the enqueuer typed it — this queue does not resolve, validate or interpret it; the consumer does.
--
-- NO EXTENSION IS CREATED. `gen_random_uuid()` is core since Postgres 13 and 62 existing migrations already use it
-- with no `pgcrypto`; adding the extension would be a second, privileged way to say the same thing.

begin;

create table if not exists forge_work_queue (
    id uuid primary key default gen_random_uuid(),

    -- What the job is and which Storyboard story it sends through Forge.
    job_name text not null,
    story_id text not null,

    -- Which vendor runs the turn, and the model or Maestro agent it runs as.
    harness text not null
        check (harness in ('opencode', 'maestro')),
    execution_target text not null,

    state text not null default 'Pending'
        check (state in ('Pending', 'Running', 'Complete', 'Error')),

    -- Lower claims first; equal priority is FIFO by creation.
    priority integer not null default 100,

    -- `attempts` counts claims, and is incremented by the claim itself, so a worker that dies mid-turn has its
    -- attempt on the row and the retry path can see it.
    attempts integer not null default 0,
    max_attempts integer not null default 5,

    claimed_by text,
    claimed_at timestamptz,

    -- A retry moves this forward; `forge_claim_next_work` refuses a row that is not yet available.
    available_at timestamptz not null default now(),

    started_at timestamptz,
    completed_at timestamptz,

    commit_sha text,
    last_error text,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- The claim's own key, restricted to the only rows a claim can return.
create index if not exists forge_work_queue_pending_idx
    on forge_work_queue (priority, created_at, id)
    where state = 'Pending';

-- ============================================================
-- ENQUEUE — the one door a new job enters through.
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
    perform pg_notify('forge_work_queue', v_id::text);

    return v_id;
end;
$$;

-- ============================================================
-- CLAIM NEXT FIFO JOB — safe for concurrent workers.
-- ============================================================

create or replace function forge_claim_next_work(
    p_worker_id text
)
returns setof forge_work_queue
language sql
as $$
    with next_job as (
        select id
        from forge_work_queue
        where state = 'Pending'
          and available_at <= now()
          and attempts < max_attempts
        order by
            priority asc,
            created_at asc,
            id asc
        for update skip locked
        limit 1
    )
    update forge_work_queue q
       set state = 'Running',
           claimed_by = p_worker_id,
           claimed_at = now(),
           started_at = coalesce(started_at, now()),
           attempts = attempts + 1,
           updated_at = now()
      from next_job n
     where q.id = n.id
    returning q.*;
$$;

-- ============================================================
-- COMPLETE — settle a Running row, carrying the commit it produced.
-- ============================================================

create or replace function forge_complete_work(
    p_id uuid,
    p_commit_sha text default null
)
returns void
language sql
as $$
    update forge_work_queue
       set state = 'Complete',
           commit_sha = p_commit_sha,
           completed_at = now(),
           updated_at = now()
     where id = p_id
       and state = 'Running';
$$;

-- ============================================================
-- RETRY — back to the FIFO after a delay, or terminal once attempts are spent.
-- ============================================================

create or replace function forge_retry_work(
    p_id uuid,
    p_error text,
    p_delay_seconds integer default 30
)
returns void
language sql
as $$
    update forge_work_queue
       set state =
               case
                   when attempts >= max_attempts then 'Error'
                   else 'Pending'
               end,
           available_at =
               case
                   when attempts >= max_attempts then available_at
                   else now() + make_interval(secs => p_delay_seconds)
               end,
           claimed_by = null,
           claimed_at = null,
           last_error = p_error,
           completed_at =
               case
                   when attempts >= max_attempts then now()
                   else null
               end,
           updated_at = now()
     where id = p_id;
$$;

-- ============================================================
-- PERMANENT FAILURE — terminal Error, no further claim.
-- ============================================================

create or replace function forge_fail_work(
    p_id uuid,
    p_error text
)
returns void
language sql
as $$
    update forge_work_queue
       set state = 'Error',
           last_error = p_error,
           completed_at = now(),
           updated_at = now()
     where id = p_id;
$$;

commit;
