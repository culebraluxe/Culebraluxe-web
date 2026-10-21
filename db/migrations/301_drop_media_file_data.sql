-- 301 — Drop the legacy blob column `media.file_data`.
--
-- WHAT IS DESTROYED. The PostgreSQL copy of a media file's bytes. Object storage (`object_bucket` / `object_key`,
-- migration 298) is the canonical store; migration 299 made the column nullable and object-first rows already leave
-- it NULL.
--
-- WHY IT IS SAFE. The surviving bytes are in object storage, uploaded and VERIFIED (SHA-256 + size) by the writer
-- that recorded the row. The column is dropped only after the preflight below proves ZERO file_data rows remain —
-- i.e. every legacy blob has been migrated and every remaining row is object-first. That is the "zero file_data rows"
-- justification, and it is checked, not assumed.
--
-- PREFLIGHT — BOTH QUERIES MUST RETURN 0 BEFORE THIS FILE IS APPLIED:
--
--   -- (a) no row still depends on the blob
--   select count(*) from media where file_data is not null;
--
--   -- (b) the migration tool (`web/src/bin/media_migrate.rs`) has no work left
--   select count(*) from media
--    where file_data is not null and storage_status not in ('VERIFIED', 'OBJECT_PRIMARY');
--
-- CODE PRECONDITION — the blob column can only go once NO code reads or writes it. At the time this draft was written
-- the deliberate residuals were (each must land first):
--
--   * db/src/vault/database.rs            — the Vault byte doors still select m.file_data
--   * db/src/broker_signature.rs          — the store-absent fallback load_legacy_protected_asset
--   * db/src/media/media_row.rs           — upload_standalone (blob writer) and media_bytes (MEDIA_OBJECT_STORAGE_READS=off reader)
--   * db/src/public_listing.rs            — the public site's blob reader
--   * db/src/luxesign.rs, db/src/signature/reconcile_completed.rs, db/src/vault/bind_form_to_contract.rs
--                                         — store-absent fallback inserts
--   * web/src/bin/media_migrate.rs        — the migration tool itself (delete it with this column)
--   * test fixtures that INSERT file_data — they must build object-first rows
--
-- AFTER the readers above are object-only, run the preflight, then apply. The column has no index or constraint of
-- its own (002_media.sql, 299_media_blob_nullable.sql), so the drop is a catalog change; existing rows keep their
-- object pointers untouched.
--
-- This file is a DRAFT in the retirement lane: it has NOT been applied to DEV or PRODUCTION.

begin;

set local lock_timeout = '5s';
set local statement_timeout = '30s';

alter table media drop column if exists file_data;

commit;
