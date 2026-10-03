-- 264_forge_agent_work_begin.sql
--
-- WHY: opening a run was a transaction choreographed from Rust (`ForgeEngineDao::begin_agent_work_run`,
-- db/src/forge_engine.rs): lock the claimed item, open the Story Run with the story's specification
-- snapshotted, move the item `Claimed → Running` stamped with that run, commit. That is database behaviour, so it
-- lives here now and the DAO binds two parameters and maps the row. Translated as-is — the third stored-routine
-- slice after 262 (claim) and 263 (settlement): no new column, no change to the snapshot, the guard or the envelope.
--
-- `Claimed → Running`, and nothing else. No row back means the item was not `Claimed`, so the caller does not own
-- the run it is about to start (an unconditional answer here once let an unowned run drive a story, 2026-09-29).
--
-- THE STORY RUN IS OPENED WHERE EXECUTION BEGINS (migration 025 §2), and the item is stamped with it in the same
-- transaction: an item `Running` beside a run row that does not exist is the pair half-moved. The specification is
-- copied from `storyboard_story` by the insert itself — the story row is the only source and the insert the only
-- writer — and `nullif(trim(…), '')` keeps a blank field the absence of a fact. `run_type` is the item's own
-- role/kind. `base_commit_hash` is deliberately absent: the worktree does not exist yet.
--
-- `p_execution_environment` is the run's ACTUAL target (migration 030: `DEV` / `PROD`), which only the calling
-- process knows — the database cannot tell which control plane it is — so it is the one fact passed in.
--
-- The envelope (execution policy, model policy, launch intent) comes back from the same statement that moves the
-- item, so the engine reads the durable envelope at the moment it starts executing and a launcher cannot
-- substitute its own.

begin;

create or replace function forge_begin_agent_work_run(p_work_item_id uuid, p_execution_environment text)
returns table (execution_policy text, story_run_id text, model_policy text, launch_intent text)
language plpgsql
as $$
declare
    v_run storyboard_story_run.id%type;
begin
    -- Locked before it is moved: the story cannot change under the run this opens, and a second begin on the same
    -- row is a no-op rather than a second run.
    perform 1
       from agent_work_item w
      where w.id = p_work_item_id and w.state = 'Claimed'
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
    returning w.execution_policy::text, v_run::text, w.model_policy::text, w.launch_intent::text;
end;
$$;

commit;
