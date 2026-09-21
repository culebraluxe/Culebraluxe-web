-- CulebraLuxe Portal
-- AUTH-02 — Security Audit Event — Human-Executable Bundle
-- Source: db/migrations/017_security_audit_event.sql
-- Date: 2026-08-20
--
-- Do NOT rerun migration 017 on top of this bundle.

-- ==================================================
-- PREFLIGHT — read-only
-- ==================================================

SELECT to_regclass('security_audit_event') AS audit_table;

-- ==================================================
-- 017 / AUTH-02 — security_audit_event
-- ==================================================

begin;

create table if not exists security_audit_event (
    id uuid primary key default gen_random_uuid(),
    app_user_id uuid references app_user(id) on delete set null,
    event_type text not null,
    authentication_method text,
    occurred_at timestamptz not null default now(),
    metadata jsonb not null default '{}'::jsonb
);

create index if not exists idx_security_audit_event_occurred
    on security_audit_event(occurred_at desc);

create index if not exists idx_security_audit_event_app_user
    on security_audit_event(app_user_id);

commit;

-- ==================================================
-- POSTFLIGHT — read-only
-- ==================================================

SELECT
    (SELECT to_regclass('security_audit_event') IS NOT NULL) AS table_exists,
    (SELECT count(*) FROM security_audit_event) AS event_count;
