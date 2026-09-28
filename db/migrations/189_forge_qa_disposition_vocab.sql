-- ENG-FORGE-QA-VERDICT-VOCAB-01 — one vocabulary for the writer, the constraint and the reader.
--
-- Migration 114 guarded `forge_last_qa_disposition` with only REPAIR, REPLAN and ESCALATE, but
-- `recordForgeQaPass` writes PASS (db/forge-repair-ledger.ts). Every clean-pass write was therefore
-- rejected by the guard and the column stayed null on every row: the pass signal never landed, and the
-- chain re-ran QA on every pass. This migration widens the guard to the ONE stored vocabulary, which is
-- defined once in workflow_app/forge/qa-repair-policy.ts (QA_STORED_DISPOSITIONS) and consumed by the
-- writers, the readers and the frozen proof (workflow_app/tests/qa-disposition-vocab.test.ts).
--
-- 114 is left untouched: it is already applied, and the schema_migration ledger records applies by checksum.

alter table storyboard_story
  drop constraint if exists storyboard_story_forge_qa_disposition_check;

alter table storyboard_story
  add constraint storyboard_story_forge_qa_disposition_check
  check (
    forge_last_qa_disposition is null
    or forge_last_qa_disposition in ('PASS', 'REPAIR', 'REPLAN', 'ESCALATE')
  );
