-- CulebraLuxe Portal
-- OPS-11A — Operational Issue Queue (single durable exception table).
--
-- KISS issue surface: `issue` is the ONLY table. No alert framework, no
-- escalation/subscriber/ACK/notification lifecycle, no Attention reuse.
-- It answers "what is currently wrong, risky, late, or broken?" for
-- Support/OPPS operational exception handling.
--
-- Deterministic issue-generation (db/issues.reconcileIssues) inserts OPEN rows
-- keyed by (type, domain_type, domain_id). The partial unique index below is
-- the DB backstop for "at most one active OPEN issue per condition": a RESOLVED
-- row frees the slot, so a returning condition can create fresh history.

begin;

create table if not exists issue (
    id uuid primary key default gen_random_uuid(),

    -- specific issue condition (APPRAISAL_OVERDUE, MISSING_EXECUTED_PS, ...)
    type text not null,

    severity text not null
        check (severity in ('RED', 'YELLOW', 'INFO')),

    state text not null default 'OPEN'
        check (state in ('OPEN', 'RESOLVED')),

    -- canonical domain object the issue is about ('deal' | 'task')
    domain_type text not null,
    domain_id uuid not null,

    title text not null,
    detail text,

    detected_at timestamptz not null default now(),
    resolved_at timestamptz,

    created_at timestamptz not null default now()
);

-- Queue scan: OPEN first, sorted by severity then age in the read model.
create index idx_issue_queue on issue (state, severity, detected_at);

-- Duplicate-OPEN backstop: one active OPEN issue per type+domain. This index
-- is partial so a RESOLVED row does NOT block a later re-detection.
create unique index uq_issue_open_once
    on issue (type, domain_type, domain_id)
    where state = 'OPEN';

commit;
