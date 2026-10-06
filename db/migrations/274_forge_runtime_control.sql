-- 274_forge_runtime_control.sql
--
-- WHY: every later slice is safe to land only because one row can stop new work without touching work already in
-- flight. On 2026-10-05 three PROD runs were in flight while the door's settle classifier was wrong, and there was no
-- switch that could hold the line: the only ways to stop the executor were to kill a process or unload a plist, and
-- both of those look identical to a dead worker — which is precisely what 273 exists to make visible. A queue whose
-- only brake is "make the worker look dead" cannot be operated.
--
-- WHAT IT IS: `forge_runtime_control` (one row, id = 1) and the two setters that are the only ways to write it.
--
--   paused                    stops NEW claims and NEW arms. In-flight runs finish normally and settle normally.
--   desired_worker_sha        the version the executor should be running; the worker compares it with its own build
--                             SHA and drains when they differ (S3), which is the whole deploy mechanism.
--   global_story_concurrency  the ceiling on concurrent stories, for the claim function (D5) to honour.
--
-- WHY "IN FLIGHT FINISHES". A kill switch that killed work in flight would be a way to lose a paid run rather than a
-- way to change your mind about the next one. Pause stops *new* work; it never destroys running work.
--
-- WHICH DOOR READS IT, AND WHEN. Nothing yet: this migration only writes the switch down. The enforcement lands in
-- 275, at both doors — `forge_claim_story` refuses to claim while paused, and `forge_arm_work_queue` refuses to arm —
-- because a table nothing reads is a record of intent, not a brake. Keeping the guard out of this file is deliberate:
-- reverting 274 must be able to drop the table without leaving a function that errors on a table that is gone.
--
-- WHO. Both setters require a non-blank `who`: a queue whose pause has no author is a queue nobody can ask about. The
-- name is recorded as given and not validated against any table — provenance, not authorization.
--
-- REVERSAL (D4 / 274): drop function if exists forge_set_desired_version(text, text);
--                       drop function if exists forge_set_paused(boolean, text);
--                       drop table if exists forge_runtime_control;
-- Non-destructive: one new table and two new functions. No existing table, column, routine or status is read or
-- changed, and no code path calls either setter until an operator or S1 does.

begin;

create table if not exists forge_runtime_control (
    id smallint primary key default 1 check (id = 1),
    paused boolean not null default false,
    desired_worker_sha text,
    global_story_concurrency integer not null default 2 check (global_story_concurrency between 1 and 64),
    updated_at timestamptz not null default now(),
    updated_by text not null default 'migration-274'
);

-- The row exists from the moment the table does, so a reader never has to handle "no control row" as a third state.
insert into forge_runtime_control (id) values (1)
on conflict (id) do nothing;

comment on table forge_runtime_control is
    'One row (id = 1). The operator''s brake and version pin; every write is stamped with who asked for it.';

comment on column forge_runtime_control.updated_by is
    'Who wrote this row. ''migration-274'' on the row the migration itself created: the author of the row is named.';

create or replace function forge_set_paused(p_paused boolean, p_who text)
returns forge_runtime_control
language plpgsql
as $$
declare
    v_row forge_runtime_control;
begin
    if btrim(coalesce(p_who, '')) = '' then
        raise exception 'forge_set_paused: who is required (a pause nobody signed cannot be asked about)'
            using errcode = '22023';
    end if;

    insert into forge_runtime_control as c (id, paused, updated_at, updated_by)
    values (1, coalesce(p_paused, false), now(), btrim(p_who))
    on conflict (id) do update
        set paused = excluded.paused,
            updated_at = now(),
            updated_by = excluded.updated_by
    returning * into v_row;

    return v_row;
end;
$$;

create or replace function forge_set_desired_version(p_sha text, p_who text)
returns forge_runtime_control
language plpgsql
as $$
declare
    v_row forge_runtime_control;
begin
    if btrim(coalesce(p_who, '')) = '' then
        raise exception 'forge_set_desired_version: who is required (a version pin nobody signed cannot be asked about)'
            using errcode = '22023';
    end if;
    if btrim(coalesce(p_sha, '')) = '' then
        -- Pinning the fleet to nothing would strand every worker on a version that cannot be named; refuse it rather
        -- than write a value whose only effect is to make "desired" undefined.
        raise exception 'forge_set_desired_version: sha is blank (name the version you mean)'
            using errcode = '22023';
    end if;

    insert into forge_runtime_control as c (id, desired_worker_sha, updated_at, updated_by)
    values (1, btrim(p_sha), now(), btrim(p_who))
    on conflict (id) do update
        set desired_worker_sha = excluded.desired_worker_sha,
            updated_at = now(),
            updated_by = excluded.updated_by
    returning * into v_row;

    return v_row;
end;
$$;

commit;
