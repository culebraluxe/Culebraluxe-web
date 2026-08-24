-- CulebraLuxe Portal
-- CRM-27 — Audited manual/external execution evidence.
--
-- Extends `agreement_execution` with audit metadata for the application-owned
-- manual/external execution path: how the fact was satisfied (execution_kind),
-- the authenticated actor (actor_app_user_id) and a bounded note/reason. The
-- unique(document_id, issued_version) marker remains the exactly-once backstop:
-- a manual execution either records the fact (kind = 'manual') or is a no-op
-- when the agreement version is already executed.
--
-- - Additive only: existing rows keep execution_kind default 'automatic'.
-- - Audit-safe: the marker must not disappear with a document (068/069).
-- - No provider dependency; the actor is the application user who authorized it.
-- - 067/068/069/070 were committed code only (never applied to any database), so
--   this is a standalone addition for the first DEV application.

begin;

alter table agreement_execution
    add column if not exists execution_kind text not null default 'automatic';

alter table agreement_execution
    add column if not exists actor_app_user_id text;

alter table agreement_execution
    add column if not exists note text;

commit;
