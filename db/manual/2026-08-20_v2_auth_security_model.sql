-- CulebraLuxe Portal
-- AUTH-01 — Application Security Model Foundation — Human-Executable Bundle
-- Source: db/migrations/015_auth_security_model.sql (canonical migration)
-- Date: 2026-08-20
--
-- This bundle is semantically identical to running migration 015 against a
-- base that already includes migrations 001..014.
--
-- Execution requirements:
--   - Run the PREFLIGHT section first (separately, read-only).
--   - Then run this bundle in order via the project's normal migration
--     mechanism or a controlled Neon session with human authorization.
--   - DO NOT rerun migration 015 on top of this bundle, and DO NOT rerun this
--     bundle on top of itself.

-- ==================================================
-- PREFLIGHT — RUN BEFORE THE BUNDLE (read-only)
-- ==================================================

-- A. Confirm the canonical app_user table exists. Expected: one row.
SELECT to_regclass('app_user') AS app_user_exists;

-- B. Confirm migration 015 has NOT already been applied.
--    Expected: zero rows (no role table / no account_type column yet).
SELECT to_regclass('role') AS role_table,
       EXISTS (
         SELECT 1
         FROM information_schema.columns
         WHERE table_name = 'app_user' AND column_name = 'account_type'
       ) AS account_type_column_exists;

-- ==================================================
-- 015 / AUTH-01 — Application Security Model (schema + seeds)
-- ==================================================

begin;

-- ------------------------------------------------------------
-- app_user — account model (additive; existing ids preserved)
-- ------------------------------------------------------------

alter table app_user
    add column if not exists account_type text
        not null default 'internal'
        check (account_type in ('internal', 'external'));

alter table app_user
    add column if not exists person_id uuid
        references person(id)
        on delete set null;

create index if not exists idx_app_user_person
    on app_user(person_id)
    where person_id is not null;

create index if not exists idx_app_user_account_type
    on app_user(account_type);

-- ------------------------------------------------------------
-- role — named bundle of authorities
-- ------------------------------------------------------------

create table if not exists role (
    id uuid primary key default gen_random_uuid(),
    code text not null unique,
    name text not null,
    account_type text not null
        check (account_type in ('internal', 'external')),
    description text,
    active boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- ------------------------------------------------------------
-- authority — coarse application capability
-- ------------------------------------------------------------

create table if not exists authority (
    id uuid primary key default gen_random_uuid(),
    code text not null unique,
    name text not null,
    description text,
    created_at timestamptz not null default now()
);

-- ------------------------------------------------------------
-- role_authority — many-to-many (role bundles authorities)
-- ------------------------------------------------------------

create table if not exists role_authority (
    role_id uuid not null references role(id) on delete cascade,
    authority_id uuid not null references authority(id) on delete cascade,
    created_at timestamptz not null default now(),
    primary key (role_id, authority_id)
);

create index if not exists idx_role_authority_authority
    on role_authority(authority_id);

-- ------------------------------------------------------------
-- app_user_role — actor's role assignments
-- ------------------------------------------------------------

create table if not exists app_user_role (
    app_user_id uuid not null references app_user(id) on delete cascade,
    role_id uuid not null references role(id) on delete cascade,
    assigned_at timestamptz not null default now(),
    assigned_by_user_id uuid references app_user(id) on delete set null,
    primary key (app_user_id, role_id)
);

create index if not exists idx_app_user_role_role
    on app_user_role(role_id);

-- ------------------------------------------------------------
-- Cross-type safety (Story 7)
-- ------------------------------------------------------------

create or replace function enforce_app_user_role_account_type()
returns trigger
language plpgsql
as $$
declare
    v_user_account_type text;
    v_role_account_type text;
begin
    select account_type into v_user_account_type
    from app_user
    where id = new.app_user_id;

    select account_type into v_role_account_type
    from role
    where id = new.role_id;

    if v_user_account_type is null then
        raise exception 'app_user % does not exist', new.app_user_id;
    end if;

    if v_role_account_type is null then
        raise exception 'role % does not exist', new.role_id;
    end if;

    if v_user_account_type <> v_role_account_type then
        raise exception
            'account type mismatch: app_user % is %, role % is %',
            new.app_user_id, v_user_account_type, new.role_id, v_role_account_type;
    end if;

    return new;
end;
$$;

drop trigger if exists enforce_app_user_role_account_type on app_user_role;
create trigger enforce_app_user_role_account_type
    before insert or update of app_user_id, role_id on app_user_role
    for each row execute function enforce_app_user_role_account_type();

-- ------------------------------------------------------------
-- Seeds — role
-- ------------------------------------------------------------

insert into role (code, name, account_type, description) values
    ('owner', 'Owner', 'internal', 'Brokerage principal with full operational control'),
    ('agent', 'Agent', 'internal', 'Brokerage agent managing listings, clients, and deals'),
    ('viewer', 'Viewer', 'internal', 'Read-only internal access to the portal'),
    ('client', 'Client', 'external', 'External buyer/seller with limited self-service access')
on conflict (code) do nothing;

-- ------------------------------------------------------------
-- Seeds — authority
-- ------------------------------------------------------------

insert into authority (code, name, description) values
    ('portal.read', 'Read the portal', 'Access portal dashboards and read-only views'),
    ('crm.write', 'Write CRM data', 'Create and update relationships, interactions, and tasks'),
    ('listing.write', 'Manage listings', 'Edit property facts, visibility, and media'),
    ('deal.read', 'Read deals', 'View deal workspaces and transaction status'),
    ('deal.write', 'Write deals', 'Create and update deal records and participants'),
    ('settings.read', 'Read settings', 'View users, roles, and authorities'),
    ('settings.manage', 'Manage settings', 'Change users, roles, and authorities'),
    ('external.properties.save', 'Save properties', 'Allow an external client to save properties'),
    ('external.deal.read_own', 'Read own deals', 'Allow an external client to view their own deals')
on conflict (code) do nothing;

-- ------------------------------------------------------------
-- Seeds — role → authority (deterministic, idempotent)
-- ------------------------------------------------------------

insert into role_authority (role_id, authority_id)
select r.id, a.id
from role r
cross join authority a
where (r.code, a.code) in (
    ('owner', 'portal.read'),
    ('owner', 'crm.write'),
    ('owner', 'listing.write'),
    ('owner', 'deal.read'),
    ('owner', 'deal.write'),
    ('owner', 'settings.read'),
    ('owner', 'settings.manage'),
    ('agent', 'portal.read'),
    ('agent', 'crm.write'),
    ('agent', 'listing.write'),
    ('agent', 'deal.read'),
    ('agent', 'deal.write'),
    ('viewer', 'portal.read'),
    ('viewer', 'deal.read'),
    ('client', 'external.properties.save'),
    ('client', 'external.deal.read_own')
)
on conflict (role_id, authority_id) do nothing;

commit;

-- ==================================================
-- POSTFLIGHT — read-only sanity check
-- ==================================================

-- Expect: 4 roles, 9 authorities, 16 mappings, 0 account-type mismatches.
SELECT (SELECT count(*) FROM role) AS roles,
       (SELECT count(*) FROM authority) AS authorities,
       (SELECT count(*) FROM role_authority) AS mappings;
