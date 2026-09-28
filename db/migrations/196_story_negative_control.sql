-- 196_story_negative_control.sql
--
-- A STORY CAN DECLARE ITS NEGATIVE CONTROL (FORGE-NEGATIVE-CONTROL-WIRING-01).
--
-- WHY THIS COLUMN EXISTS. ENG-FORGE-FENCE-CAN-FAIL-01 built the capability — the collector, the adjudicator
-- and the evidence columns (`negative_control_ran`, `negative_control_killing_assertion`, migration 193) —
-- and then Astra's review measured that the PRODUCTION runner never supplied it: every ordinary run left
-- `ports.negativeControl` undefined, so the capability could only be exercised by a hand-assembled test.
-- The evidence columns record the outcome; this column holds the DECLARATION, which is what the runner reads
-- in order to run the control at all. Without it, "a fence proves it can fail" was a promise no dispatch
-- could keep.
--
-- The assertions the control is expected to kill are deliberately NOT stored: the plan's acceptance mapping
-- already says which assertions the fence proves, and the adjudicator treats an absent list as "every mapped
-- assertion is intended". Storing them twice would let the two drift.
set lock_timeout = '5s';
set statement_timeout = '30s';

alter table storyboard_story
  add column if not exists negative_control_command text;

comment on column storyboard_story.negative_control_command is
  'The single command that applies this story''s inversion, runs the SAME fence, and destroys its own scratch state. NULL means the story declares no control, and QA then records that honestly (no control is not a pass).';
