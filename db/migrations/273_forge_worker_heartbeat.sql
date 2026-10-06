-- 273_forge_worker_heartbeat.sql
--
-- WHY: nothing in the database says whether a worker is alive. On 2026-10-05 the scheduled worker died on every tick
-- (`stop: checkout-not-main branch='lane/deep'` — the plist named a lane, not the trunk checkout) and Forge read as
-- idle: a dead executor and a quiet queue are the same absence, 573 rows waited behind it, and no row anywhere said
-- so. "No work" and "no worker" must be different facts, and the second one has to be queryable.
--
-- WHAT IT IS: three objects and no more.
--
--   forge_worker_heartbeat   one row per worker_id — the process's own statement about itself
--   forge_worker_beat(...)   the upsert that writes it; the only write door
--   forge_worker_health      the same rows with `stale` computed, so a reader never has to know the threshold
--
-- THE HOST IS A COLUMN, NOT AN ASSUMPTION. The Mac may run a worker (the one-shot wrapper, or `--watch`) beside the
-- server-side one, and both must be visible while neither may be silent: each carries its own `worker_id` and `host`,
-- and this table is where a second executor announces itself. Nothing here assumes the executor is the server, and
-- nothing here lets a worker run unnoticed — a worker that does not beat reads `stale`, which is a louder fact than
-- a worker nobody knew about.
--
-- `p_started_at` IS THE PROCESS'S OWN START, not the beat's timestamp: a restart must show the new process, so the
-- upsert takes the caller's value rather than keeping the oldest seen. A worker that does not track its own start
-- gets the default (`now()`), which says "I started when I last beat" — honest but weaker, and S1 passes the real one.
--
-- FIVE MINUTES is the threshold in `forge_worker_health`, matching the order's O5 alert. A worker that polls every
-- 30 s (S2) and beats once per pass is stale only if it has actually stopped, never because it was between beats.
--
-- REVERSAL (D3 / 273): drop view if exists forge_worker_health;
--                       drop function if exists forge_worker_beat(text, text, text, text, integer, text, timestamptz);
--                       drop table if exists forge_worker_heartbeat;
-- Non-destructive: three new objects, no existing table, column, routine or status is read or changed.

begin;

create table if not exists forge_worker_heartbeat (
    worker_id     text primary key,
    host          text not null,
    git_sha       text not null,
    started_at    timestamptz not null,
    last_seen_at  timestamptz not null,
    state         text not null check (state in ('idle', 'running', 'draining', 'stopping')),
    running_count integer not null default 0 check (running_count >= 0),
    last_error    text
);

comment on table forge_worker_heartbeat is
    'One row per worker process. `state` is the worker''s own word; `stale` (see forge_worker_health) is the reader''s.';

-- The upsert. `host`, `git_sha` and `state` are the three facts a reader uses to decide whether to trust the row, so
-- all three are required: a beat that cannot name where it runs, what it runs and what it is doing is not a beat.
create or replace function forge_worker_beat(
    p_worker_id text,
    p_host text,
    p_git_sha text,
    p_state text,
    p_running_count integer default 0,
    p_last_error text default null,
    p_started_at timestamptz default now()
)
returns forge_worker_heartbeat
language plpgsql
as $$
declare
    v_row forge_worker_heartbeat;
begin
    if btrim(coalesce(p_worker_id, '')) = '' then
        raise exception 'forge_worker_beat: worker_id is blank' using errcode = '22023';
    end if;
    if btrim(coalesce(p_host, '')) = '' then
        raise exception 'forge_worker_beat: host is blank (a beat that cannot name its host cannot be trusted)'
            using errcode = '22023';
    end if;
    if btrim(coalesce(p_git_sha, '')) = '' then
        raise exception 'forge_worker_beat: git_sha is blank (report the build you are, not nothing)'
            using errcode = '22023';
    end if;

    insert into forge_worker_heartbeat as h (
        worker_id, host, git_sha, started_at, last_seen_at, state, running_count, last_error
    )
    values (
        btrim(p_worker_id),
        btrim(p_host),
        btrim(p_git_sha),
        coalesce(p_started_at, now()),
        now(),
        p_state,
        greatest(coalesce(p_running_count, 0), 0),
        nullif(btrim(coalesce(p_last_error, '')), '')
    )
    on conflict (worker_id) do update
        set host = excluded.host,
            git_sha = excluded.git_sha,
            started_at = excluded.started_at,
            last_seen_at = excluded.last_seen_at,
            state = excluded.state,
            running_count = excluded.running_count,
            last_error = excluded.last_error
    returning * into v_row;

    return v_row;
end;
$$;

-- The reader's view of the same rows. `stale` is computed here rather than at every call site, so O5, an operator and
-- a test all answer "is the executor alive" with one rule.
create or replace view forge_worker_health as
select h.worker_id,
       h.host,
       h.git_sha,
       h.started_at,
       h.last_seen_at,
       h.state,
       h.running_count,
       h.last_error,
       h.last_seen_at < now() - interval '5 minutes' as stale,
       floor(extract(epoch from (now() - h.last_seen_at)))::bigint as age_seconds
  from forge_worker_heartbeat h;

comment on view forge_worker_health is
    'Worker liveness. `stale` is last_seen_at older than five minutes: no fresh beat means no worker, whatever the queue says.';

commit;
