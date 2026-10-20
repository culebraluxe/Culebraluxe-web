-- 298_media_object_storage_metadata.sql
--
-- WHY: Phase 2 of the media move to Neon Object Storage. These columns let a media row describe an
-- object without moving or deleting a single byte of the existing `file_data` blob. Existing rows stay
-- LEGACY_BLOB; the application keeps reading blobs exactly as before. No blob column is altered.
--
-- VISIBILITY: existing rows default to `private` because the current database has no per-media
-- visibility column. Public listing photos are classified by the Phase 5/6 backfill before any direct
-- public URL is issued. A default of private cannot leak an unpublished photo.
--
-- STORAGE STATUS: LEGACY_BLOB / MIRROR_PENDING / MIRRORED / VERIFIED / OBJECT_PRIMARY / MIGRATION_FAILED.
-- Only VERIFIED/OBJECT_PRIMARY may ever be used for object reads; the legacy blob remains the fallback.
BEGIN;

alter table media
    add column if not exists storage_status text not null default 'LEGACY_BLOB',
    add column if not exists visibility text not null default 'private',
    add column if not exists object_bucket text,
    add column if not exists object_key text,
    add column if not exists object_etag text,
    add column if not exists object_sha256 text,
    add column if not exists object_size bigint,
    add column if not exists object_migrated_at timestamptz;

alter table media
    drop constraint if exists media_storage_status_check;
alter table media
    add constraint media_storage_status_check check (
        storage_status in (
            'LEGACY_BLOB', 'MIRROR_PENDING', 'MIRRORED', 'VERIFIED', 'OBJECT_PRIMARY',
            'MIGRATION_FAILED'
        )
    );

alter table media
    drop constraint if exists media_visibility_check;
alter table media
    add constraint media_visibility_check check (visibility in ('public', 'private', 'vault'));

alter table media
    drop constraint if exists media_object_size_check;
alter table media
    add constraint media_object_size_check check (object_size is null or object_size >= 0);

alter table media
    drop constraint if exists media_object_sha256_check;
alter table media
    add constraint media_object_sha256_check check (
        object_sha256 is null or object_sha256 ~ '^[0-9a-f]{64}$'
    );

alter table media
    drop constraint if exists media_object_key_check;
alter table media
    add constraint media_object_key_check check (
        object_key is null
        or (length(object_key) between 1 and 1024 and object_key not like '/%' and object_key not like '%/')
    );

-- One object key is one row. The partial index keeps legacy rows (NULL key) untouched.
create unique index if not exists media_object_identity_uidx
    on media (object_bucket, object_key)
    where object_key is not null;

create index if not exists media_storage_status_idx on media (storage_status);

comment on column media.storage_status is
    'LEGACY_BLOB until the object is verified; VERIFIED/OBJECT_PRIMARY are the only object-read states.';
comment on column media.visibility is
    'public = published direct URL; private = signed URL; vault = signed URL after Vault authorization.';
comment on column media.object_sha256 is
    'SHA-256 of the object bytes, persisted after verification.';

COMMIT;
