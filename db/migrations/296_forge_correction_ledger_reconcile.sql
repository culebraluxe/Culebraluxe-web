-- Reconcile the correction ledger after a post-apply rewrite of migration 295.
--
-- WHY THIS FILE EXISTS (documented exception, no 295 edit):
--   295_forge_correction_ledger.sql was rewritten AFTER it was applied to DEV and PROD. The
--   schema_migration ledger therefore records the checksum of the applied version
--   (sha256:36a53a77d28cd606ec303153a5edd5fd5aaa875f56cccf39e2856e25ecac8500) while the file on
--   disk hashes to sha256:1ba6058691afb86a9f43cddecd4ba74344a0b3ecd08a72c05e99d12d22c6beb0.
--   Editing 295 again would rewrite history a second time and could never satisfy both ledgers, so
--   295 is FROZEN. This additive migration is the reconciliation record: it re-asserts the
--   reconciled shape idempotently, so a database that applied the older 295 and a database that
--   applies the current file end in the same schema.
--
-- WHAT THE BOOT GATE DOES WITH THIS: db/src/boot_gate.rs keeps 295 in the required list (it is
-- applied everywhere) and does NOT gate on its checksum — for entries in RECONCILED_AFTER_APPLY it
-- verifies the SCHEMA (required columns) instead. Do not add 296 to REQUIRED_BOOT_MIGRATIONS: it is
-- a reconciliation note, not a code dependency, and PROD has not applied it yet.
--
-- Additive only, idempotent, no data mutation. Deliberately NOT applied by the build lane:
-- migrations are human holds (WO-07).

begin;

alter table if exists forge_correction_ledger
    add column if not exists lane text,
    add column if not exists correction_key text,
    add column if not exists correction text,
    add column if not exists context text,
    add column if not exists root_cause text,
    add column if not exists spec_location text,
    add column if not exists permanent_rule_created boolean not null default false,
    add column if not exists evidence_refs jsonb not null default '[]'::jsonb,
    add column if not exists experiment_id text,
    add column if not exists created_at timestamp with time zone not null default now();

create index if not exists idx_forge_correction_ledger_repeat
    on forge_correction_ledger (lane, correction_key);
create index if not exists idx_forge_correction_ledger_lane
    on forge_correction_ledger (lane, created_at desc);
create index if not exists idx_forge_correction_ledger_open
    on forge_correction_ledger (created_at desc)
    where not permanent_rule_created;
create index if not exists idx_forge_correction_ledger_experiment
    on forge_correction_ledger (experiment_id)
    where experiment_id is not null;

comment on table forge_correction_ledger is
    'Forge correction ledger (migration 295). The 295 file was rewritten after apply; the boot gate verifies this table''s schema, not the 295 checksum. Migration 296 is the additive reconciliation record.';

commit;
