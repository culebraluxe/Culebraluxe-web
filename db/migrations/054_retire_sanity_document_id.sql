-- CulebraLuxe Portal
-- PLAT-01 Property Source Consolidation — retire the last Sanity schema artifact
-- Migration: 054_retire_sanity_document_id.sql
--
-- The Sanity `property` document was retired as the property source; Neon is
-- the single canonical source (commit f0aa00f). `property.sanity_document_id`
-- (migration 001) is the only remaining Sanity column: no code reads it, and
-- the DEV branch holds zero non-null values. Dropping it completes the schema
-- cleanup. Sanity identity is not relationship identity — `property.id` is the
-- stable identity (AGENTS.md).
--
-- Applied to the disposable DEV branch only. PRODUCTION promotion happens only
-- through an explicit production-release task.

begin;

alter table property
    drop constraint if exists property_sanity_document_unique;

alter table property
    drop column if exists sanity_document_id;

commit;
