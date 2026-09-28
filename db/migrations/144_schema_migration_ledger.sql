-- CulebraLuxe
-- Migration ledger. The missing piece that let DEV and PROD drift silently.
-- Migration: 144_schema_migration_ledger.sql
--
-- Before this, `apply-migration.mjs` executed a SQL file and recorded NOTHING,
-- so "what is applied where?" was unanswerable. That is how PROD went weeks
-- without 116-122/138 while DEV went weeks without the Forge dispatch columns.
--
-- Honest history note: ledger records are authoritative only from the baseline
-- forward. Pre-baseline application history for this project is unknown, so a
-- single '<baseline>' row per target documents the verified parity state
-- (2026-09-10: DEV and PROD identical across tables, columns, indexes and FKs).

begin;

create table if not exists schema_migration (
    id            uuid primary key default gen_random_uuid(),
    filename      text        not null,
    checksum      text        not null,
    target        text        not null check (target in ('dev', 'prod', 'baseline')),
    note          text,
    applied_at    timestamptz not null default now(),
    constraint schema_migration_filename_target_unique unique (filename, target)
);

create index if not exists idx_schema_migration_applied_at on schema_migration (applied_at desc);
create index if not exists idx_schema_migration_target on schema_migration (target);

comment on table schema_migration is
    'Applied-migration ledger. Written by scripts/apply-migration.mjs. Authoritative from the 2026-09-10 baseline forward; pre-baseline rows are backfilled/reconstructed.';
comment on column schema_migration.checksum is
    'sha256 of the migration file at apply time; a differing checksum means the file changed after it was applied.';

commit;
