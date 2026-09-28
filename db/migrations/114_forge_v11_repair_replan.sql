-- ENG-FORGE-V11-S1 — durable QA repair/replan lifecycle ledger.
--
-- Story completion/repair routing must be derived from durable state, not a
-- process-local executor variable. These columns live on the canonical story
-- row (where markForgeStory* already records Forge lifecycle truth) so attempt
-- counts and the last QA disposition survive process restart, stale-worker
-- recovery, task reclaim, and executor restart.
--
-- Bounded autonomy (V11 §1.6): repair/replan budgets are constants enforced in
-- the policy module (qa-repair-policy.ts). The DB enforces atomic single-row
-- increments; the engine is authoritative for legality.

alter table storyboard_story
  add column if not exists forge_repair_attempts integer not null default 0,
  add column if not exists forge_replan_attempts integer not null default 0,
  add column if not exists forge_last_qa_disposition text,
  add column if not exists forge_last_failure_reason text;

-- Disposition vocabulary guard: only legal machine dispositions (or NULL when
-- QA has not failed) may be recorded.
alter table storyboard_story
  drop constraint if exists storyboard_story_forge_qa_disposition_check;

alter table storyboard_story
  add constraint storyboard_story_forge_qa_disposition_check
  check (
    forge_last_qa_disposition is null
    or forge_last_qa_disposition in ('REPAIR', 'REPLAN', 'ESCALATE')
  );
