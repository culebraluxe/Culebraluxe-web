-- 188_sprint_accounting.sql
--
-- WHAT A SPRINT COST AND HOW LONG IT TOOK, FED BY WHAT THE ENGINE ALREADY RECORDED.
--
-- Nothing new is written by the lanes for this. `storyboard_story_run` has carried
-- started_at/ended_at, tokens_input/output, cost_usd (real vendor USD, rare), cost_widgets (Forge's
-- standardized quantity) and cost_source ('vendor' | 'widgets') since migrations 107 and 133. This
-- migration sums them up the parent, and it is a VIEW rather than a maintained total for one blunt
-- reason: a stored total is a second copy of a fact that can drift, and the sprint tables in 187
-- refuse exactly that for sprint_id. A view is free AND cannot be wrong.
--
-- THREE HONESTY RULES ARE BUILT IN, BECAUSE EACH ONE HAS ALREADY BITTEN SOMETHING TONIGHT:
--
--   1. USD AND WIDGETS ARE NEVER ADDED TOGETHER. They are not convertible here: cost_usd is what the
--      vendor invoiced, cost_widgets is model_weight x elapsed minutes. One total would be a number
--      nobody could act on. They are summed separately, and `runs_with_source` counts the runs that
--      carry either.
--
--   2. EVERY TOTAL CARRIES ITS COVERAGE. Measured 2026-09-17: 435 of 1075 runs have vendor USD, 278
--      have widgets, 362 have no cost at all. A bare "$11.14" reads like the sprint's cost when it is
--      the cost of the two thirds we have. The view reports the denominators beside the sums, so the
--      reader can see what the number rests on.
--
--   3. A NUMBER THAT DOES NOT EXIST IS NULL, NOT ZERO. A sprint with no runs has no elapsed time; a
--      story never completed has no cycle time. Zero would claim a measurement nobody made, exactly
--      as 0% would for an empty sprint (187).
--
-- THE ONE TRIGGER HERE FREEZES THE NUMBERS AT CLOSE, and it exists because cost arrives LATE: the
-- vendor invoices after the fact (migration 133 says so), and runs can be re-run. Without a
-- snapshot, a sprint's closed accounting would keep moving after everyone has read the outcome. So
-- the parent keeps both truths and labels them: the live view answers "what do we know now", and
-- `closed_*` answers "what we knew when it closed". One-shot on the transition into Closed, so a
-- second close never rewrites the first.
--
-- Non-destructive: one new view family and one added set of nullable columns. No existing table,
-- column or row is touched.

-- THE CHILD FEEDS THE PARENT: one row per story, summed from its runs.
create or replace view storyboard_story_accounting as
select
    st.id as story_id,
    st.sprint_id,
    count(r.id) as runs,
    count(r.cost_source) as runs_with_cost,
    count(r.cost_usd) as runs_with_usd,
    count(r.cost_widgets) as runs_with_widgets,
    count(r.ended_at) as runs_with_end,
    coalesce(sum(r.cost_usd), 0) as cost_usd,
    coalesce(sum(r.cost_widgets), 0) as cost_widgets,
    coalesce(sum(r.tokens_input), 0) as tokens_input,
    coalesce(sum(r.tokens_output), 0) as tokens_output,
    coalesce(sum(extract(epoch from (r.ended_at - r.started_at))), 0) as run_seconds,
    min(r.started_at) as first_run_at,
    max(r.ended_at) as last_run_end_at,
    st.actual_start_at,
    st.completed_at,
    case
        when st.completed_at is not null and st.actual_start_at is not null
        then extract(epoch from (st.completed_at - st.actual_start_at))
    end as cycle_seconds
from storyboard_story st
left join storyboard_story_run r on r.story_id = st.id
group by st.id, st.sprint_id, st.actual_start_at, st.completed_at;

comment on view storyboard_story_accounting is
    'One story with what its runs cost and how long they took, straight from storyboard_story_run. cycle_seconds is null unless the story both started and completed (188).';
comment on column storyboard_story_accounting.cost_usd is
    'Vendor USD only. Never add this to cost_widgets: the units are different and not convertible (188).';
comment on column storyboard_story_accounting.runs_with_cost is
    'Runs carrying a cost of some kind (cost_source is not null), whether vendor USD or widgets (188).';

-- THE PARENT SUMS ITS CHILDREN, AND CARRIES THE DENOMINATORS WITH THE SUMS.
create or replace view storyboard_sprint_accounting as
select
    s.id as sprint_id,
    s.number,
    coalesce(sum(a.runs), 0) as runs,
    coalesce(sum(a.runs_with_cost), 0) as runs_with_cost,
    coalesce(sum(a.runs_with_usd), 0) as runs_with_usd,
    coalesce(sum(a.runs_with_widgets), 0) as runs_with_widgets,
    coalesce(sum(a.runs_with_end), 0) as runs_with_end,
    sum(a.cost_usd) as cost_usd,
    sum(a.cost_widgets) as cost_widgets,
    sum(a.tokens_input) as tokens_input,
    sum(a.tokens_output) as tokens_output,
    sum(a.run_seconds) as run_seconds,
    min(a.first_run_at) as first_run_at,
    max(a.last_run_end_at) as last_run_end_at,
    case
        when min(a.first_run_at) is not null and max(a.last_run_end_at) is not null
        then extract(epoch from (max(a.last_run_end_at) - min(a.first_run_at)))
    end as wall_seconds,
    count(a.cycle_seconds) as stories_with_cycle,
    avg(a.cycle_seconds) as mean_cycle_seconds
from storyboard_sprint s
left join storyboard_story_accounting a on a.sprint_id = s.id
group by s.id, s.number;

comment on view storyboard_sprint_accounting is
    'What a sprint cost and how long its lanes ran, summed from its stories runs. cost_usd and cost_widgets are separate units and are never added together; every sum has its run counts beside it so the coverage is visible (188).';
comment on column storyboard_sprint_accounting.run_seconds is
    'Summed lane time across runs (ended_at - started_at). Wall time is separate: wall_seconds is first run start to last run end and includes the waiting (188).';
comment on column storyboard_sprint_accounting.mean_cycle_seconds is
    'Mean completed story cycle time. Null unless a story carried BOTH actual_start_at and completed_at; stories_with_cycle says how many that was (188).';

-- ONE READ SURFACE FOR THE PARENT: the story rollup from 187 and the accounting, joined.
create or replace view storyboard_sprint_board as
select
    r.id,
    r.number,
    r.title,
    r.theme,
    r.status,
    r.owner,
    r.goal,
    r.started_at,
    r.target_end_at,
    r.closed_at,
    r.outcome,
    r.stories,
    r.stories_complete,
    r.stories_open,
    r.stories_held,
    r.percent_complete,
    a.runs,
    a.runs_with_cost,
    a.runs_with_usd,
    a.runs_with_widgets,
    a.cost_usd,
    a.cost_widgets,
    a.tokens_input,
    a.tokens_output,
    a.run_seconds,
    a.wall_seconds,
    a.stories_with_cycle,
    a.mean_cycle_seconds,
    a.first_run_at,
    a.last_run_end_at
from storyboard_sprint_rollup r
join storyboard_sprint_accounting a on a.sprint_id = r.id;

comment on view storyboard_sprint_board is
    'The sprint parent as the board reads it: story counts plus what it cost and how long it ran (188).';


-- WHAT WE KNEW WHEN IT CLOSED. These columns are the reason a trigger is right here and wrong for the
-- live sum: cost_usd is filled in by the vendor after the fact, so without a snapshot a closed
-- sprint's accounting keeps moving. The live view answers "what do we know now"; these answers
-- "what did we know then", and both are labelled.
alter table storyboard_sprint
    add column if not exists closed_snapshot_at timestamptz,
    add column if not exists closed_runs bigint,
    add column if not exists closed_runs_with_cost bigint,
    add column if not exists closed_cost_usd numeric(12, 6),
    add column if not exists closed_cost_widgets numeric,
    add column if not exists closed_tokens_input bigint,
    add column if not exists closed_tokens_output bigint,
    add column if not exists closed_run_seconds numeric,
    add column if not exists closed_wall_seconds numeric;

comment on column storyboard_sprint.closed_snapshot_at is
    'When the closed_* accounting was frozen. A closed sprint with no snapshot was closed before 188 existed; that is unknown, not zero (188).';
comment on column storyboard_sprint.closed_cost_usd is
    'Vendor USD as known at close. The live figure lives in storyboard_sprint_accounting and will usually be larger, because invoices arrive late (188).';

-- The snapshot itself, so it can be taken deliberately (`pnpm sprint snapshot`) as well as on close.
create or replace function storyboard_sprint_snapshot(p_sprint_id text)
returns void language plpgsql as $$
declare
    a record;
begin
    select * into a from storyboard_sprint_accounting where sprint_id = p_sprint_id;
    update storyboard_sprint
    set closed_snapshot_at = now(),
        closed_runs = coalesce(a.runs, 0),
        closed_runs_with_cost = coalesce(a.runs_with_cost, 0),
        closed_cost_usd = a.cost_usd,
        closed_cost_widgets = a.cost_widgets,
        closed_tokens_input = a.tokens_input,
        closed_tokens_output = a.tokens_output,
        closed_run_seconds = a.run_seconds,
        closed_wall_seconds = a.wall_seconds
    where id = p_sprint_id;
end
$$;

comment on function storyboard_sprint_snapshot(text) is
    'Freeze the live accounting onto the sprint row. Called by the close trigger, and callable deliberately to re-take a snapshot (188).';

-- ONE-SHOT ON THE TRANSITION INTO Closed. `old.status is distinct from 'Closed'` means a second close
-- leaves the first snapshot alone: history is not rewritten by a later update. An explicit
-- storyboard_sprint_snapshot() call is the deliberate way to refresh it.
create or replace function storyboard_sprint_snapshot_on_close()
returns trigger language plpgsql as $$
begin
    if new.status = 'Closed' and old.status is distinct from 'Closed' then
        perform storyboard_sprint_snapshot(new.id);
    end if;
    return new;
end
$$;

comment on function storyboard_sprint_snapshot_on_close() is
    'Takes the accounting snapshot once, when a sprint first becomes Closed (188).';

drop trigger if exists storyboard_sprint_snapshot_on_close on storyboard_sprint;
create trigger storyboard_sprint_snapshot_on_close
    after update of status on storyboard_sprint
    for each row execute function storyboard_sprint_snapshot_on_close();

