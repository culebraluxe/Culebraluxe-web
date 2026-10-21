-- 299_media_blob_nullable.sql
--
-- Object-first uploads no longer need to write a Postgres copy. The legacy `file_data` column stays
-- during the transition (old rows retain it), but new object-first rows are inserted without bytes.
-- A later cleanup migration drops the column once no reader/writer selects it.
BEGIN;
alter table media alter column file_data drop not null;
COMMENT ON COLUMN media.file_data IS
    'Legacy blob. NULL for object-first rows; retained only until the final blob-column cleanup.';
COMMIT;
