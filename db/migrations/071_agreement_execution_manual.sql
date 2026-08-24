-- CulebraLuxe Portal
-- CRM-27 — Audited manual/external execution evidence.
--
-- Extends `agreement_execution` with audit metadata for the application-owned
-- manual/external execution path: how the fact was satisfied (execution_kind),
-- the authenticated actor (actor_app_user_id, UUID FK to app_user) and a bounded
-- note/reason. The unique(document_id, issued_version) marker remains the
-- exactly-once backstop: a manual execution either records the fact
-- (kind = 'manual') or is a no-op when the agreement version is already executed.
--
-- Audit integrity constraints:
--   - execution_kind is constrained to automatic | manual;
--   - actor_app_user_id is UUID with an FK to app_user(id) using audit-safe
--     RESTRICT delete (an app_user who authorized a manual execution cannot be
--     deleted out from under the audit record, and the manual-requires-actor
--     invariant cannot be violated by a SET NULL cascade);
--   - a manual execution REQUIRES an actor (engine-automatic rows may have none);
--   - note is bounded to 500 characters.
--
-- - Additive only: existing rows keep execution_kind default 'automatic'.
-- - No provider dependency; the actor is the application user who authorized it.
-- - 067/068/069/070 were committed code only (never applied to any database), so
--   this is corrected IN PLACE for the first DEV application.

begin;

alter table agreement_execution
    add column if not exists execution_kind text not null default 'automatic';

alter table agreement_execution
    add column if not exists actor_app_user_id uuid;

alter table agreement_execution
    add column if not exists note text;

drop constraint if exists agreement_execution_kind_check;
alter table agreement_execution
    add constraint agreement_execution_kind_check
    check (execution_kind in ('automatic', 'manual'));

drop constraint if exists agreement_execution_actor_fk;
alter table agreement_execution
    add constraint agreement_execution_actor_fk
    foreign key (actor_app_user_id) references app_user(id) on delete restrict;

drop constraint if exists agreement_execution_actor_required_for_manual;
alter table agreement_execution
    add constraint agreement_execution_actor_required_for_manual
    check (execution_kind <> 'manual' or actor_app_user_id is not null);

drop constraint if exists agreement_execution_note_length;
alter table agreement_execution
    add constraint agreement_execution_note_length
    check (note is null or char_length(note) <= 500);

commit;
