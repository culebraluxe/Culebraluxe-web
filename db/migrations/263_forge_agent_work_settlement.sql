-- 263_forge_agent_work_settlement.sql
--
-- WHY: settling a claim was a transaction choreographed from Rust (`ForgeEngineDao::finish_agent_work_run` and
-- `reject_agent_work_configuration`, db/src/forge_engine.rs), with the pair it writes chosen by a Rust
-- policy (`settlement_pair`, `run_result_status_for`) and the run closed by a Rust helper (`close_story_run_in`).
-- All of it is database behaviour, so all of it lives here now and the DAO binds parameters and maps the row.
-- Translated as-is, the second stored-routine slice after 262 (the claim): no new column, no change to any state,
-- status, reason text or guard.
--
-- A SETTLED CLAIM AND ITS STORY ARE ONE FACT WITH TWO ROWS, and `forge_settlement_pair` is the only place the pair
-- is chosen. The board moves only while it still expects a run (`Ready` / `In Progress`): `Complete` is the truth
-- and no failure demotes it, `Hold` already needs a human, `Planned` / `Batched` belong to reset and staging.
--   * `Done` is accepted only when the board confirms it (`Complete`) or a human gate ended the turn (`Hold`).
--     Otherwise it is REFUSED: the item records `Error`, the story is held, and the reason names the board status.
--     The engine returns Ok for runs the board does not call finished (exhausted, the step cap, a blocked wave).
--   * `Abandoned` is the engine's own fault: nothing about the story was decided. While the board expects a run
--     both halves go back to `Ready` — item first, so the dispatch trigger's `on conflict do nothing` sees the open
--     item. Over a board that no longer expects a run, the item is `Cancelled` and the board is left alone.
--   * `Error` / `Cancelled` are written as such, holding a board that still expects a run.
--
-- A cleared claim is not a free retry: after `max_attempts` (default 3) cleared runs the pair is settled as an
-- `Error` instead, so a broken engine stops spinning one story through the queue (2026-09-29).
--
-- The run a claim opened closes in the same transaction, guarded by `ended_at is null`. A cleared claim rules
-- nothing: its run's `result_status` stays NULL, because an engine fault is not a story verdict.

begin;

-- The pair a settled claim must leave behind. `p_outcome` is `Done` | `Error` | `Cancelled` | `Abandoned`.
create or replace function forge_settlement_pair(
    p_outcome text,
    p_board text,
    out item_state text,
    out story_status text,
    out reason text
)
language plpgsql
immutable
as $$
declare
    v_board_belongs_to_a_run boolean := p_board in ('Ready', 'In Progress');
    v_hold_the_board text := case when p_board in ('Ready', 'In Progress') then 'Hold' end;
begin
    if p_outcome = 'Done' and p_board in ('Complete', 'Hold') then
        item_state := 'Done';
    elsif p_outcome = 'Done' then
        item_state := 'Error';
        story_status := v_hold_the_board;
        reason := format(
            'run ended without the board confirming completion (story status ''%s''); Done refused', p_board);
    elsif p_outcome = 'Abandoned' and v_board_belongs_to_a_run then
        item_state := 'Ready';
        story_status := 'Ready';
    elsif p_outcome = 'Abandoned' then
        item_state := 'Cancelled';
    elsif p_outcome in ('Error', 'Cancelled') then
        item_state := p_outcome;
        story_status := v_hold_the_board;
    else
        raise exception 'forge_settlement_pair: unknown outcome %', p_outcome using errcode = '22023';
    end if;
end;
$$;

-- The ruling an item's terminal state gives its Story Run. `Ready` (a cleared claim) rules nothing.
create or replace function forge_run_result_status_for(p_item_state text)
returns text
language sql
immutable
as $$
    select case p_item_state
        when 'Done' then 'Complete'
        when 'Error' then 'Failed'
        when 'Cancelled' then 'Cancelled'
    end
$$;

-- Close the Story Run a claim opened. A second settle cannot rewrite a ruling; a claim that opened no run is a
-- no-op. A blank reason leaves the notes alone, otherwise it is appended on its own line.
create or replace function forge_close_story_run(p_work_item_id uuid, p_ruling text, p_reason text)
returns void
language sql
as $$
    update storyboard_story_run r
       set ended_at = now(),
           result_status = p_ruling,
           completion = case when p_ruling = 'Complete' then 100 else r.completion end,
           notes = case
               when nullif(trim(coalesce(p_reason, '')), '') is null then r.notes
               when r.notes is null or r.notes = '' then p_reason
               else r.notes || E'\n' || p_reason
           end,
           updated_at = now()
     where r.id = (select w.story_run_id from agent_work_item w where w.id = p_work_item_id)
       and r.ended_at is null
$$;

-- The one terminal write: a claim ends exactly once. `state in ('Claimed','Running')` is the guard that makes a
-- second settle a no-op. Returns the settlement written, or no row when this call settled nothing.
create or replace function forge_finish_agent_work_run(p_work_item_id uuid, p_outcome text, p_error_text text)
returns table (item_state text, story_status text, reason text)
language plpgsql
as $$
declare
    v_board text;
    v_attempts integer;
    v_max_attempts integer;
    v_pair record;
    v_reason text;
    v_changed integer;
begin
    select s.status, i.attempts, coalesce(i.max_attempts, 3)
      into v_board, v_attempts, v_max_attempts
      from agent_work_item i
      join storyboard_story s on s.id = i.story_id
     where i.id = p_work_item_id and i.state in ('Claimed', 'Running')
       for update of i;
    if not found then
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

    -- A cleared row is back in the queue, so its claim is unset with it.
    if v_pair.item_state = 'Ready' then
        update agent_work_item w
           set state = 'Ready', error_text = v_reason, claimed_at = null, claimed_by = null, started_at = null,
               finished_at = null, updated_at = now()
         where w.id = p_work_item_id and w.state in ('Claimed', 'Running');
    else
        update agent_work_item w
           set state = v_pair.item_state, error_text = v_reason, finished_at = now(), updated_at = now()
         where w.id = p_work_item_id and w.state in ('Claimed', 'Running');
    end if;
    get diagnostics v_changed = row_count;
    if v_changed = 0 then
        return;
    end if;

    if v_pair.story_status is not null then
        update storyboard_story s
           set status = v_pair.story_status, completed_at = null, updated_at = now()
         where s.id = (select w.story_id from agent_work_item w where w.id = p_work_item_id);
    end if;

    perform forge_close_story_run(p_work_item_id, forge_run_result_status_for(v_pair.item_state), v_reason);

    item_state := v_pair.item_state;
    story_status := v_pair.story_status;
    reason := v_pair.reason;
    return next;
end;
$$;

-- Terminalize a claim that never became a run because its configuration is unusable: `Error`, the board held
-- with it while it expects a run, and the run closed unruled with the refusal as its note.
create or replace function forge_reject_agent_work_configuration(p_work_item_id uuid, p_evidence text)
returns void
language plpgsql
as $$
declare
    v_board text;
    v_pair record;
    v_changed integer;
begin
    select s.status
      into v_board
      from agent_work_item i
      join storyboard_story s on s.id = i.story_id
     where i.id = p_work_item_id and i.state in ('Claimed', 'Ready')
       for update of i;
    if not found then
        return;
    end if;

    select * into v_pair from forge_settlement_pair('Error', v_board);
    update agent_work_item w
       set state = v_pair.item_state, error_text = p_evidence, finished_at = now(), updated_at = now()
     where w.id = p_work_item_id and w.state in ('Claimed', 'Ready');
    get diagnostics v_changed = row_count;
    if v_changed = 0 then
        return;
    end if;

    if v_pair.story_status is not null then
        update storyboard_story s
           set status = v_pair.story_status, completed_at = null, updated_at = now()
         where s.id = (select w.story_id from agent_work_item w where w.id = p_work_item_id);
    end if;

    perform forge_close_story_run(p_work_item_id, null, p_evidence);
end;
$$;

commit;
