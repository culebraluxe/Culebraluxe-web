-- FORGE-B3: typed acceptance plans are operator-approved on the Story and copied
-- immutably into each Story Run before execution begins. Existing rows stay NULL:
-- legacy prose is not promoted into fabricated structured proof.

set lock_timeout = '5s';
set statement_timeout = '30s';

alter table storyboard_story
    add column if not exists assay_plan jsonb,
    add column if not exists assay_plan_approved_by text,
    add column if not exists assay_plan_approved_at timestamptz,
    add column if not exists assay_plan_approved_hash text;

alter table storyboard_story_run
    add column if not exists assay_plan_snapshot jsonb;

alter table forge_workflow_evidence
    add column if not exists role_output_schema_version bigint,
    add column if not exists role_output_diagnostic text;

create or replace function forge_capture_assay_plan_snapshot()
returns trigger
language plpgsql
as $$
declare
    v_plan jsonb;
    v_approved_by text;
    v_approved_at timestamptz;
    v_approved_hash text;
begin
    if tg_op = 'UPDATE' then
        if new.assay_plan_snapshot is distinct from old.assay_plan_snapshot then
            raise exception 'assay_plan_snapshot is immutable for Story Run %', old.id
                using errcode = '23514';
        end if;
        return new;
    end if;

    select s.assay_plan, s.assay_plan_approved_by, s.assay_plan_approved_at,
           s.assay_plan_approved_hash
      into v_plan, v_approved_by, v_approved_at, v_approved_hash
      from storyboard_story s
     where s.id = new.story_id;

    new.assay_plan_snapshot := jsonb_build_object(
        'plan', v_plan,
        'approved_by', v_approved_by,
        'approved_at', v_approved_at,
        'approved_hash', v_approved_hash
    );
    return new;
end;
$$;

drop trigger if exists storyboard_story_run_assay_plan_snapshot on storyboard_story_run;
create trigger storyboard_story_run_assay_plan_snapshot
before insert or update of assay_plan_snapshot on storyboard_story_run
for each row execute function forge_capture_assay_plan_snapshot();

create or replace function forge_invalidate_assay_plan_approval()
returns trigger
language plpgsql
as $$
begin
    if new.assay_plan is distinct from old.assay_plan
       and new.assay_plan_approved_hash is not distinct from old.assay_plan_approved_hash then
        new.assay_plan_approved_by := null;
        new.assay_plan_approved_at := null;
        new.assay_plan_approved_hash := null;
    end if;
    return new;
end;
$$;

drop trigger if exists storyboard_story_assay_plan_approval on storyboard_story;
create trigger storyboard_story_assay_plan_approval
before update of assay_plan on storyboard_story
for each row execute function forge_invalidate_assay_plan_approval();

comment on column storyboard_story.assay_plan is
    'FORGE-B3 schema-v1 typed acceptance plan proposal. Only an operator-approved hash can be frozen into a run.';
comment on column storyboard_story.assay_plan_approved_hash is
    'SHA-256 of canonical schema-v1 plan bytes approved by a human; changed plans invalidate the approval.';
comment on column storyboard_story_run.assay_plan_snapshot is
    'Immutable plan and approval provenance copied at run creation; NULL plan means legacy plan-required.';
comment on column forge_workflow_evidence.role_output_schema_version is
    'Schema version of the latest accepted typed role-output marker for this process instance.';
comment on column forge_workflow_evidence.role_output_diagnostic is
    'Latest typed role-output acceptance or rejection reason, including the producing node.';

-- A reused key is idempotent only when it describes the same immutable receipt. The earlier
-- implementation returned the first row for every same-key retry, even when the retry carried
-- different measurement content. Preserve the existing single artifact write boundary and make
-- that mismatch a visible provenance conflict.
create or replace function forge_record_tool_artifact(
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
    v_existing forge_tool_artifact%rowtype;
    v_stored_verdict text;
begin
    if p_story_run_id is not null then
        select r.result_status into v_ruling from storyboard_story_run r where r.id = p_story_run_id;
    end if;
    v_stored_verdict := forge_artifact_verdict_for_run(p_kind, v_ruling, p_verdict);

    if p_idempotency_key is not null then
        select a.* into v_existing
          from forge_tool_artifact a
         where a.idempotency_key = p_idempotency_key;
        if found then
            if v_existing.story_id is distinct from p_story_id
               or v_existing.story_run_id is distinct from p_story_run_id
               or v_existing.tool is distinct from p_tool
               or v_existing.kind is distinct from p_kind
               or v_existing.verdict is distinct from v_stored_verdict
               or v_existing.summary is distinct from p_summary
               or v_existing.detail is distinct from p_detail
               or v_existing.sha is distinct from p_sha then
                raise exception 'forge_tool_artifact idempotency key % has conflicting content', p_idempotency_key
                    using errcode = '23505';
            end if;
            return query
            select v_existing.id::text, v_existing.story_id, v_existing.story_run_id::text,
                   v_existing.tool, v_existing.kind, v_existing.verdict, v_existing.summary,
                   v_existing.sha, v_existing.created_at::text;
            return;
        end if;
    end if;

    begin
        return query
        insert into forge_tool_artifact as a
            (story_id, story_run_id, tool, kind, verdict, summary, detail, sha, idempotency_key)
        values
            (p_story_id, p_story_run_id, p_tool, p_kind, v_stored_verdict,
             p_summary, p_detail, p_sha, p_idempotency_key)
        returning a.id::text, a.story_id::text, a.story_run_id::text, a.tool::text,
                  a.kind::text, a.verdict::text, a.summary::text, a.sha::text, a.created_at::text;
    exception when unique_violation then
        if p_idempotency_key is null then
            raise;
        end if;
        select a.* into v_existing
          from forge_tool_artifact a
         where a.idempotency_key = p_idempotency_key;
        if not found
           or v_existing.story_id is distinct from p_story_id
           or v_existing.story_run_id is distinct from p_story_run_id
           or v_existing.tool is distinct from p_tool
           or v_existing.kind is distinct from p_kind
           or v_existing.verdict is distinct from v_stored_verdict
           or v_existing.summary is distinct from p_summary
           or v_existing.detail is distinct from p_detail
           or v_existing.sha is distinct from p_sha then
            raise exception 'forge_tool_artifact idempotency key % has conflicting content', p_idempotency_key
                using errcode = '23505';
        end if;
        return query
        select v_existing.id::text, v_existing.story_id, v_existing.story_run_id::text,
               v_existing.tool, v_existing.kind, v_existing.verdict, v_existing.summary,
               v_existing.sha, v_existing.created_at::text;
    end;
end;
$$;
