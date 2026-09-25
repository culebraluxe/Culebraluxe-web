-- 221 — The manifest for a chunked upload: what is being uploaded, before it exists.
--
-- WHY A MANIFEST AND NOT JUST CHUNKS. `media_upload_chunk` (220) holds bytes keyed by upload id, but bytes with no
-- owner cannot be validated, cannot be swept by age with a reason, and cannot be assembled into the right row: the
-- filename, mime type, declared size and destination Property all have to be known when the first chunk arrives,
-- which is before `media` has a row to point at. One row per upload carries that.
--
-- THE DECLARED SIZE IS A PROMISE THE SERVER CHECKS. `byte_size` and `chunk_count` come from the client, and
-- `complete` refuses unless every expected chunk is present and the assembled length matches — so a truncated upload
-- cannot be mistaken for a photo, and a client cannot quietly send 2 GB by lying about the count.
--
-- `sha256` is the whole file's digest, recorded at init and verified at complete. It is what makes "the bytes that
-- arrived are the bytes that were chosen" a fact rather than a hope, and it is the field that lets a future resumable
-- client prove it already delivered a chunk.
set lock_timeout = '5s';
set statement_timeout = '30s';

create table if not exists media_upload (
    upload_id uuid primary key,
    property_id uuid not null,
    filename text not null,
    mime_type text not null,
    byte_size bigint not null check (byte_size > 0),
    chunk_count int not null check (chunk_count > 0),
    chunk_size int not null check (chunk_size > 0),
    sha256 text,
    role text not null default 'gallery' check (role in ('hero', 'gallery')),
    alt_text text,
    status text not null default 'uploading' check (status in ('uploading', 'complete', 'failed')),
    created_at timestamptz not null default now()
);

-- The sweep reads by age, and by state: an upload stuck at 'uploading' is the leftover to clean.
create index if not exists media_upload_created_at_idx on media_upload (created_at);

comment on table media_upload is
    'Manifest for a chunked media upload: declared size and chunk count (checked at complete), destination Property, and the whole-file digest. Rows are deleted when the upload completes; stale uploading rows are swept.';
