-- 300 — Drop the retired chunk-staging tables (`media_upload_chunk`, `media_upload`).
--
-- WHAT IS DESTROYED. The two tables that staged a chunked browser upload: one row per declared upload and one row
-- per received piece. Nothing else references them.
--
-- WHY IT IS SAFE. They are TRANSIENT STAGING, not committed data: `media_upload_chunk.bytes` are opaque pieces of a
-- photograph that does not exist yet, and a completed upload deletes both rows as part of the transaction that writes
-- the real `media` row. The only rows that can still exist are abandoned uploads (a closed tab), which are garbage —
-- the sweep path removed them by age. The chunked protocol is RETIRED in the same change set: `web/src/api/routes.rs`
-- no longer registers `/v1/properties/{id}/media/uploads*`, the portal no longer registers
-- `/api/property-media/chunked`, the service/repository/DAO methods are deleted, and the browser uploads small
-- photographs whole (`/api/property-media/full`) and large ones direct to object storage
-- (`/api/property-media/direct/*`). Zero runtime code references either table.
--
-- PREFLIGHT (run against the target and read the answer; a non-zero second count means someone is still staging an
-- upload from an old browser bundle — wait for the sweep rather than dropping rows an upload is using):
--
--   select count(*) as uploads from media_upload;
--   select count(*) as chunks from media_upload_chunk;
--   select count(*) as fresh_uploads
--     from media_upload
--    where created_at > now() - interval '1 day';
--
-- ORDER. The child table first, then the manifest. `media_upload_chunk` carries no foreign key to
-- `media_upload`, so the order is convention, not a constraint; it is kept because it mirrors how the rows were
-- written and read.
--
-- Apply only through the one migration applier, and only after the preflight above is understood. This file is a
-- DRAFT in the retirement lane: it has NOT been applied to DEV or PRODUCTION.

begin;

set local lock_timeout = '5s';
set local statement_timeout = '30s';

drop table if exists media_upload_chunk;
drop table if exists media_upload;

commit;
