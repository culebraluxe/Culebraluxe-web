-- 182_story_completion_requires_complete.sql
--
-- ASTRA ITEM 6 (2026-09-16). `completion = 100` while the status said Hold or In Progress was true on FOUR rows
-- (249 were honest), and nothing checked the pair, so the lie survived in the board for months. The rule belongs
-- where the data lives: a completion of 100 is a claim that the story IS complete, so the database refuses the
-- pair rather than trusting every future writer to remember.
--
-- The four offending rows were corrected to 0 BEFORE this migration, and deliberately NOT promoted to Complete —
-- promoting would have invented a completion. The constraint therefore applies to clean data.
--
-- Deliberately NOT receipt-gated: the release record has zero rows, so requiring an eligible release receipt
-- here would freeze every future completion. This is the rule half; the receipt requirement follows once a real
-- release has produced a row.
--
-- Idempotent: safe to re-run.

alter table storyboard_story
  drop constraint if exists storyboard_story_completion_requires_complete;

alter table storyboard_story
  add constraint storyboard_story_completion_requires_complete
  check (completion is null or completion < 100 or status = 'Complete');

comment on constraint storyboard_story_completion_requires_complete on storyboard_story is
  'A story may only read 100% completion when its status is Complete (ASTRA item 6, 2026-09-16).';
