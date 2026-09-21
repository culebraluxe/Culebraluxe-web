-- CulebraLuxe Portal
-- AUTH-05 — Sensitive Administrative Write Audit
-- Migration: 039_security_audit_admin_action.sql
--
-- Additive: security_audit_event (reused, not a sibling table) gains the
-- structured actor/action/resource/outcome shape for allow-listed
-- ADMIN_ACTION rows (settings mutations, identity link/unlink, admin
-- reset/reconcile). Break-glass rows keep their existing columns untouched;
-- these columns are nullable and only populated for event_type = 'ADMIN_ACTION'.
-- metadata jsonb remains available for extra non-secret context. NEVER store
-- secrets, tokens, notes or PII beyond the minimal resource reference
-- (app_user_id + resource_type/resource_id).

begin;

alter table security_audit_event
    add column if not exists action text,
    add column if not exists resource_type text,
    add column if not exists resource_id uuid,
    add column if not exists outcome text,
    add column if not exists request_id text;

create index if not exists idx_security_audit_event_action
    on security_audit_event(action, occurred_at desc);

commit;
