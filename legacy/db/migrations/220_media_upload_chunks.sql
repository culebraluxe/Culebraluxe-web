-- 220 — Staged chunks for large photo uploads.
--
-- WHY. Vercel's gateway refuses any request body over ~4.5 MB. Measured, not assumed: a 6 MB body to the Rust
-- container (a separate Vercel project, not a serverless function) returns
--   413  Request Entity Too Large  FUNCTION_PAYLOAD_TOO_LARGE
-- so the cap is on the platform's edge, and moving the bytes to a different Vercel route would not lift it. A listing
-- photograph from a phone is 8–12 MB, and these are photographs of million-dollar houses: shrinking them is not the
-- answer. The browser sends the file in pieces, each comfortably under the cap, and the server reassembles it.
--
-- THE PIECES ARE OPAQUE. Nothing reads, serves or thumbnails a chunk. The assembled file is written to
-- media.file_data by ONE insert — the same `insert into media (...)` that already exists — so no partial image can
-- ever be visible, the serving path does not change, and a half-finished upload leaves no trace in the gallery.
--
-- STAGED IN THE DATABASE, NOT IN MEMORY. On Vercel each chunk can land on a different instance and the Rust
-- container may be running more than one, so "keep part 1 in memory until part 2 arrives" would pass a test and
-- then corrupt a real upload under load. Persisting each chunk also makes a retried chunk safe rather than
-- duplicated: the primary key is the (upload, index) pair.
--
-- NOT AN OBJECT STORE. The bytes are stored in Neon, in the media table, because that is where they already live and
-- the whole catalogue is small (tens of properties, thousands of images at most) — an object store would be new
-- machinery for a problem we do not have.
set lock_timeout = '5s';
set statement_timeout = '30s';

create table if not exists media_upload_chunk (
    upload_id uuid not null,
    chunk_index int not null check (chunk_index >= 0),
    bytes bytea not null,
    created_at timestamptz not null default now(),
    primary key (upload_id, chunk_index)
);

-- The sweep reads by age: an upload abandoned at chunk 1 of 4 must not sit in Neon forever.
create index if not exists media_upload_chunk_created_at_idx on media_upload_chunk (created_at);

comment on table media_upload_chunk is
    'Staging for chunked image uploads. Opaque bytes only: nothing serves or inspects a partial upload, and rows are deleted when the upload completes or expires.';
