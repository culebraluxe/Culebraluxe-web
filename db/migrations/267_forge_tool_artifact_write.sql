-- 267_forge_tool_artifact_write.sql
--
-- WHY: the one write of `forge_tool_artifact` (migration 130) was a transaction choreographed from Rust
-- (`ForgeEngineDao::record_tool_artifact`, db/src/forge_engine.rs): read the run's ruling, decide which
-- verdict the artifact may carry with a Rust policy (`artifact_verdict_for_run`, `verdict_polarity`), insert. The
-- policy only ever compared database values, so all of it lives here now. Translated as-is — the sixth
-- stored-routine slice after 262–266.
--
-- AN ARTIFACT CARRIES A RULING, NEVER A SECOND OPINION. For `kind = 'run-verdict'` the verdict is kept only when its
-- POLARITY agrees with the run's `result_status`:
--
--   | run ruling                     | artifact verdict | kept     |
--   | Complete                       | PASS             | PASS     |
--   | Hold                           | Failed           | Failed   |
--   | Complete                       | Hold             | — (contradiction; the summary stays, the verdict not) |
--   | unruled (NULL, or no polarity) | Failed           | —        |
--
-- `pass`/`passed`/`complete`/`completed` affirm, `fail`/`failed`/`hold` deny, compared trimmed and case-blind; any
-- other word claims no polarity and two claims that cannot be compared never agree. Any other `kind` is that tool's
-- own reading (an assay's PASS is a measurement, not a claim about the run) and passes through untouched. A cleared
-- run's `result_status` is NULL (migration 263) and certifies nothing.
--
-- Fidelity note: Rust trimmed Unicode whitespace and lowercased ASCII; this trims the ASCII whitespace set and uses
-- `lower()`. They agree on every token that can equal one of the seven polarity words.

begin;

-- `Affirm`, `Negative`, or NULL for a word that names neither.
create or replace function forge_verdict_polarity(p_token text)
returns text
language sql
immutable
as $$
    select case lower(btrim(p_token, E' \t\n\r\f\v'))
        when 'pass' then 'Affirm'
        when 'passed' then 'Affirm'
        when 'complete' then 'Affirm'
        when 'completed' then 'Affirm'
        when 'fail' then 'Negative'
        when 'failed' then 'Negative'
        when 'hold' then 'Negative'
    end
$$;

-- The verdict an artifact of `p_kind` may carry under a run ruled `p_ruling`.
create or replace function forge_artifact_verdict_for_run(p_kind text, p_ruling text, p_verdict text)
returns text
language sql
immutable
as $$
    select case
        when p_verdict is null then null
        when lower(btrim(p_kind, E' \t\n\r\f\v')) is distinct from 'run-verdict' then p_verdict
        when forge_verdict_polarity(p_verdict) is null then null
        when forge_verdict_polarity(p_ruling) is null then null
        when forge_verdict_polarity(p_ruling) = forge_verdict_polarity(p_verdict) then p_verdict
    end
$$;

-- The one write of `forge_tool_artifact`. The ruling is read from the run row in the same transaction, never taken
-- from the caller.
create or replace function forge_record_tool_artifact(
    p_story_id text,
    p_story_run_id uuid,
    p_tool text,
    p_kind text,
    p_verdict text,
    p_summary text,
    p_detail jsonb,
    p_sha text
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
    if p_story_run_id is not null then
        select r.result_status into v_ruling from storyboard_story_run r where r.id = p_story_run_id;
    end if;

    return query
    insert into forge_tool_artifact as a (story_id, story_run_id, tool, kind, verdict, summary, detail, sha)
    values (p_story_id, p_story_run_id, p_tool, p_kind,
            forge_artifact_verdict_for_run(p_kind, v_ruling, p_verdict), p_summary, p_detail, p_sha)
    returning a.id::text, a.story_id::text, a.story_run_id::text, a.tool::text, a.kind::text,
              a.verdict::text, a.summary::text, a.sha::text, a.created_at::text;
end;
$$;

commit;
