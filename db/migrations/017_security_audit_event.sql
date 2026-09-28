-- CulebraLuxe Portal
-- AUTH-02 — Security Audit Event
-- Migration: 017_security_audit_event.sql
--
-- Durable, deliberately separate security audit trail (NOT the CRM
-- interaction history). Captures security-significant events such as
-- break-glass root login success. Failed attempts are intentionally NOT
-- persisted here (infrastructure-log only) to avoid credential/DoS side
-- effects.

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
