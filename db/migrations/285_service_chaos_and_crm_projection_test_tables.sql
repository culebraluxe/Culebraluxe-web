-- Keep the database-backed integration-test support tables present on both
-- environments. Tests historically created these lazily on DEV, which left
-- schema parity reporting a DEV-only table/index drift.

begin;

create table if not exists chaos_service_test_writes (
    write_key text primary key,
    write_value text not null,
    attempt integer not null default 1,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

comment on table chaos_service_test_writes is
    'Integration-test fixture for transient service write failures.';

create table if not exists crm_lead_projection (
    lead_id text primary key,
    projection_json jsonb not null,
    deal_count integer not null default 0,
    total_value numeric not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

comment on table crm_lead_projection is
    'Integration-test fixture for CRM lead projection contract checks.';

commit;
