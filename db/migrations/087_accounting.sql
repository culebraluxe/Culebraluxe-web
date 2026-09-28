-- ===========================================================================
-- ACCOUNTING V1 — canonical brokerage receivable + expense tables.
--
-- Mirrors the EXISTING DEV schema (tables were created in DEV first). Idempotent
-- so it is a no-op where they already exist and creates them on a fresh DB so
-- source control matches DEV. Reuses canonical CRM objects via FKs (deal,
-- property, person, app_user). NO GL/chart, journal, AP, or P&L persistence.
-- ===========================================================================

create table if not exists public.account_receivable (
    id uuid primary key default gen_random_uuid(),
    reference text,
    description text not null,
    category text not null default 'COMMISSION',
    amount numeric not null check (amount >= 0),
    issued_on date not null default current_date,
    due_on date,
    status text not null default 'OPEN'
        check (status in ('OPEN', 'PAID', 'VOID')),
    paid_on date,
    deal_id uuid references deal(id) on delete set null,
    property_id uuid references property(id) on delete set null,
    person_id uuid references person(id) on delete set null,
    created_by_user_id uuid references app_user(id) on delete set null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    -- A receivable can only be PAID when we capture when it was paid.
    check (status <> 'PAID' or paid_on is not null)
);

create table if not exists public.account_expense (
    id uuid primary key default gen_random_uuid(),
    vendor text not null,
    category text not null,
    amount numeric not null check (amount >= 0),
    expense_on date not null default current_date,
    status text not null default 'POSTED'
        check (status in ('DRAFT', 'POSTED', 'VOID')),
    memo text,
    deal_id uuid references deal(id) on delete set null,
    property_id uuid references property(id) on delete set null,
    person_id uuid references person(id) on delete set null,
    created_by_user_id uuid references app_user(id) on delete set null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
