-- FORGE-B2 slice 1: fail closed for older claimed or paused generations whose
-- model-attempt history predates the durable ledger.

begin;

insert into forge_model_attempt_budget (story_run_id, cap, used)
select distinct r.id, 100, 100
  from storyboard_story_run r
  join agent_work_item w on w.story_run_id = r.id
 where w.state in ('Claimed', 'Paused')
on conflict (story_run_id) do update
   set used = greatest(forge_model_attempt_budget.used, forge_model_attempt_budget.cap),
       updated_at = now();

commit;
