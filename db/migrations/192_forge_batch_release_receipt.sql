-- 192_forge_batch_release_receipt.sql
--
-- A SPRINT RELEASE RECORDS WHAT IT CARRIED (ENG-FORGE-BATCH-RECEIPT-01).
--
-- A story may complete QA-verified with its deployment DEFERRED to a release batch
-- (`forge_workflow_evidence.deployment_deferred_to_batch`). The captain runs that
-- batch release by hand (`scripts/vercel-release-prod.sh` prints the source commit it
-- built and deployed), and until now nothing recorded what the release actually did:
-- `scripts/forge-batch-release.mjs` lists the slice and says so — "refuses to pretend a
-- deployment happened". That refusal is right; this migration is the other half.
--
-- WHY NEW COLUMNS ON `forge_workflow_evidence` AND NOT A REUSE: the per-story deploy
-- path already owns `deployment_receipt` and `deployed_sha`. Writing a batch release
-- into those would give one fact two writers, which is exactly the failure the one-writer
-- rule exists to prevent. These three columns are a NEW fact — the batch release receipt —
-- and `recordForgeBatchReleaseReceipt` (db/forge-workflow-evidence.ts) is their only writer.
--
-- WHY NULLABLE, NO DEFAULT: a batch with no release has no receipt. NULL is "no release
-- recorded", never a fabricated commit; a default would invent a release nobody performed.
-- The batch number itself is not stored here — it is already the row's recorded
-- `deployment_deferred_to_batch`, and one fact is spelled once.
--
-- Non-destructive: three added nullable columns. No existing row, column or constraint is
-- touched.

-- SET BEFORE THE DDL. A migration that waits on a lock queues every query behind it, so it can take
-- production down without failing. Squawk refused this file on both rules the hour CI went live
-- (2026-09-18, run 35343996263); nothing has applied 192, so this is a fix and not a rewrite.
set lock_timeout = '5s';
set statement_timeout = '30s';

alter table forge_workflow_evidence
    add column if not exists batch_released_sha text,
    add column if not exists batch_released_at timestamptz,
    add column if not exists batch_release_receipt text;

comment on column forge_workflow_evidence.batch_released_sha is
    'The commit an ACTUAL batch release built and deployed for this story''s deferred batch. Null until a release happened; never asserted from the deferral (ENG-FORGE-BATCH-RECEIPT-01).';
comment on column forge_workflow_evidence.batch_released_at is
    'When the batch release happened. Null until a release happened (ENG-FORGE-BATCH-RECEIPT-01).';
comment on column forge_workflow_evidence.batch_release_receipt is
    'The release receipt identity, batch-release:<batch>:<sha>. One writer: recordForgeBatchReleaseReceipt (ENG-FORGE-BATCH-RECEIPT-01).';

-- CONCURRENTLY IS NOT AVAILABLE TO THIS REPOSITORY, stated rather than papered over:
-- scripts/apply-migration.mjs runs an entire migration file as one `pool.query(sql)`, and PostgreSQL
-- executes a multi-statement simple query as a SINGLE implicit transaction, inside which
-- CREATE INDEX CONCURRENTLY is refused outright. No migration in this repository uses CONCURRENTLY
-- for exactly that reason. The table is the engine's own evidence table and is small, so this lock is
-- short; the durable fix is an applier that can run a statement outside a transaction, which belongs
-- to ENG-FORGE-MIGRATION-LINT-01 rather than to this file.
-- squawk-ignore require-concurrent-index-creation
create index if not exists forge_workflow_evidence_batch_release_idx
    on forge_workflow_evidence (deployment_deferred_to_batch, batch_released_at)
    where batch_release_receipt is not null;
