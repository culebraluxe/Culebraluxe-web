-- 271_forge_work_queue_event_settle.sql
--
-- WHY. 270 made the door row follow its story, but only where the sweep finds it: `forge_settle_work_queue()` runs
-- once per pass, from `forge_reconcile_dispatch_queue()` (265). Between the moment a run ends and that pass, a row
-- whose run is over still reads `Running` — and `Running` is exactly what `arm_limit` counts (270:83-86), so the
-- sweep that would settle the row is the same sweep that refuses to arm behind it.
--
-- THE JAM (PROD, 2026-10-05 23:54-23:57). Three runs ended `Done` over a board still `In Progress`, so
-- `forge_settlement_pair` (263) refused the `Done`: the item recorded `Error`, the story was held, and the run was
-- closed `Failed` with the refusal as its note. The story is then neither `Complete` (270's branch 1) nor `Failed`
-- (branch 2) — `Hold` is the one status a refused `Done` writes — so no settle branch matched: all three rows stayed
-- `Running`, `v_inflight (3) >= arm_limit (3)` held, and `forge_arm_work_queue` returned 0 on every pass after that.
-- The inlet was closed by its own pacing, with three stories still holding their slots. (PROD, read before this
-- migration: three rows `Running`, three items `Error`, three stories `Hold`, three instances `error`.)
--
-- WHY THE ITEM IS THE KEY. `agent_work_item.state` is the one column that says an attempt is over, and
-- `forge_finish_agent_work_run` (263) is its only writer. A story's `status` is a board fact that a reset, a staging
-- pass or a human gate may move afterwards, and `Failed` is a status the settlement never writes for a refused
-- `Done`. The classifier therefore reads the story's latest item and calls the attempt over, instead of inferring the
-- attempt's fate from the board. The reason it carries is that item's own `error_text`: the settlement's words,
-- unparaphrased.
--
-- WHAT IS DELIBERATELY NOT SETTLED HERE. A latest item that is `Error` / `Cancelled` over a story that is back on
-- the board (`Planned` / `Ready`) is not a failure at the door: branch 3 owns it and returns the row to the FIFO. A
-- story that is `Complete` is branch 1's. And a story with a live engine instance is nobody's until that instance
-- ends, so this branch carries branch 3's instance guard verbatim.
--
-- THE EVENT HALF, AND WHY IT IS GUARDED. The same settlement is now also called by the writer that ends the attempt
-- (`forge_finish_agent_work_run`, 263:159), inside the transaction that settles the claim: a run that ends no longer
-- leaves its slot held until some later pass. Both callers read the same columns, so this is a second *caller*, not a
-- second adjudicator, and it never races the sweep — `for update of q skip locked` (270:102) already made concurrent
-- passes safe. The two calls carry their own `exception when others then raise warning` blocks — SEPARATELY, because a
-- plpgsql exception block is a subtransaction and one block around both would let a failing arm undo a settle that had
-- already succeeded. So a door that cannot settle costs one `warning` line and one sweep of latency (the sweep is the
-- backstop), never the claim settlement that just happened. Measured by injection in the DEV smoke: with
-- `forge_arm_work_queue` replaced by a raising body, the finish still returned its row, the claim settlement was kept,
-- the arm's failure was a single warning line, and the door row still settled.
--
-- REVERSAL. This file replaces two routines whose previous definitions are complete in the migrations that created
-- them: `forge_settle_work_queue` in `270_forge_work_queue_inlet.sql:132-207` and `forge_finish_agent_work_run` in
-- `263_forge_agent_work_settlement.sql:100-166`. Re-running either file restores its own definition and nothing else
-- here exists to undo.
--
-- Non-destructive: two routines replaced. No table, column, constraint, index, status, guard or reason text is read
-- differently, and 269's and 270's objects are untouched.

begin;

-- ============================================================
-- SETTLE — 270's four branches, plus the attempt that ended badly.
-- ============================================================

create or replace function forge_settle_work_queue()
returns bigint
language plpgsql
as $$
declare
    v_settled bigint := 0;
    v_rows bigint;
    v_row record;
begin
    -- 1. The story finished: the row completes, carrying the run's commit and what actually ran.
    update forge_work_queue q
       set state = 'Complete',
           commit_sha = (
               select r.commit_hash from storyboard_story_run r
                where r.story_id = q.story_id and r.ended_at is not null
                order by r.ended_at desc limit 1),
           ran_as = (
               select r.model_used from storyboard_story_run r
                where r.story_id = q.story_id and r.ended_at is not null
                order by r.ended_at desc limit 1),
           completed_at = now(),
           updated_at = now()
     where q.state = 'Running'
       and exists (select 1 from storyboard_story s where s.id = q.story_id and s.status = 'Complete');
    get diagnostics v_rows = row_count;
    v_settled := v_settled + v_rows;

    -- 2. The story failed: terminal at the door, with the engine's own reason where it left one.
    for v_row in
        select q.id, q.story_id
          from forge_work_queue q
          join storyboard_story s on s.id = q.story_id
         where q.state = 'Running' and s.status = 'Failed'
    loop
        perform forge_fail_work(
            v_row.id,
            coalesce(
                (select s.forge_last_failure_reason from storyboard_story s where s.id = v_row.story_id),
                'story ' || v_row.story_id || ' reached Failed'));
        v_settled := v_settled + 1;
    end loop;

    -- 2b. The attempt ended badly and the board does not expect another one. The story's latest claim is terminal
    --     (`Error` / `Cancelled` — 263's settlement is its only writer), so no run is coming for this row, and the
    --     reason is the settlement's own words. Without this branch a refused `Done` (board still `In Progress`,
    --     story held) keeps its slot forever and the inlet closes on its own pacing: the PROD jam above.
    for v_row in
        select q.id, q.story_id, w.state as item_state, w.error_text
          from forge_work_queue q
          join storyboard_story s on s.id = q.story_id
          join lateral (
              select w2.state, w2.error_text
                from agent_work_item w2
               where w2.story_id = q.story_id
               order by w2.created_at desc, w2.id desc
               limit 1) w on true
         where q.state = 'Running'
           and w.state in ('Error', 'Cancelled')
           and s.status not in ('Planned', 'Ready', 'Complete')
           and not exists (select 1 from process_instances p
                            where p.subject_type = 'story' and p.subject_id = q.story_id
                              and p.status in ('active', 'running', 'reserved', 'suspended'))
    loop
        perform forge_fail_work(
            v_row.id,
            coalesce(
                nullif(btrim(coalesce(v_row.error_text, '')), ''),
                format('run for story %s ended %s', v_row.story_id, v_row.item_state)));
        v_settled := v_settled + 1;
    end loop;

    -- 3. The run ended without settling: the story is back on the board and nothing holds it. The engine's own
    -- sweep is already retrying that story, so the row goes back to the FIFO rather than sitting `Running`.
    update forge_work_queue q
       set state = 'Pending',
           claimed_by = null,
           claimed_at = null,
           available_at = now() + interval '30 seconds',
           last_error = 'run ended without settling',
           updated_at = now()
     where q.state = 'Running'
       and exists (select 1 from storyboard_story s
                    where s.id = q.story_id and s.status in ('Planned', 'Ready'))
       and not exists (select 1 from agent_work_item w
                        where w.story_id = q.story_id
                          and w.state in ('Ready', 'Claimed', 'Running', 'Paused'))
       and not exists (select 1 from process_instances p
                        where p.subject_type = 'story' and p.subject_id = q.story_id
                          and p.status in ('active', 'running', 'reserved', 'suspended'));
    get diagnostics v_rows = row_count;
    v_settled := v_settled + v_rows;

    -- 4. Attempts spent: nothing will claim it again, so it must not keep reading as Pending.
    update forge_work_queue q
       set state = 'Error',
           last_error = coalesce(q.last_error, 'attempts exhausted'),
           completed_at = now(),
           updated_at = now()
     where q.state = 'Pending' and q.attempts >= q.max_attempts;
    get diagnostics v_rows = row_count;
    v_settled := v_settled + v_rows;

    return v_settled;
end;
$$;

-- ============================================================
-- THE ATTEMPT'S OWN WRITER — 263's settlement, and now also the door's event settle.
-- ============================================================

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

    -- THE DOOR SETTLES HERE, IN THIS TRANSACTION (270). The row this attempt was armed from follows its story the
    -- moment the attempt ends, and the slot it held is free for the next Pending row — which is also what arms it,
    -- so a run that ends never leaves the inlet closed behind it for a whole pass. Both routines are 270's, called
    -- with the sweep's own arguments; nothing here is a second verdict on the story, because both read the columns
    -- this settlement has already written.
    --
    -- GUARDED ON PURPOSE, AND IN TWO BLOCKS. The settlement above has happened: a door that cannot settle must not
    -- cost it. A failure inside the door is a `warning` and one sweep of latency (the sweep is the backstop), never
    -- a rolled-back claim — the difference between a jam that clears itself and a story that must run again.
    --
    -- TWO blocks, because a plpgsql exception block is a SUBTRANSACTION: its writes are undone when it raises, so
    -- one block around both calls lets a failing ARM discard a SETTLE that had already succeeded. Measured on DEV
    -- (2026-10-06, smoke §4): with the arm replaced by a raising body, one block left the door row `Running`; split,
    -- the settle stands and the row goes `Error`. The claim settlement is outside both blocks and is kept either way.
    begin
        perform forge_settle_work_queue();
    exception when others then
        raise warning 'forge_finish_agent_work_run: door settle failed for item % (sqlstate %, %); the claim settlement is kept and the sweep will retry',
            p_work_item_id, sqlstate, sqlerrm;
    end;

    begin
        perform forge_arm_work_queue(null, 'forge-settle');
    exception when others then
        raise warning 'forge_finish_agent_work_run: door arm failed for item % (sqlstate %, %); the settle above is kept and the sweep will retry',
            p_work_item_id, sqlstate, sqlerrm;
    end;

    item_state := v_pair.item_state;
    story_status := v_pair.story_status;
    reason := v_pair.reason;
    return next;
end;
$$;

commit;
