-- CulebraLuxe Portal
-- AUTH-02 (schema portion) — Provider-Neutral Identity Mapping — Human-Executable Bundle
-- Source: db/migrations/016_auth_identity.sql (canonical migration)
-- Date: 2026-08-20
--
-- This bundle is semantically identical to running migration 016 against a
-- base that already includes migrations 001..015.
--
-- Execution requirements:
--   - Run the PREFLIGHT section first (separately, read-only).
--   - Then run the migration section in order via the project's normal
--     migration mechanism or a controlled Neon session with human authorization.
--   - DO NOT rerun migration 016 on top of this bundle, and DO NOT rerun this
--     bundle on top of itself.

-- ==================================================
-- PREFLIGHT — RUN BEFORE THE BUNDLE (read-only)
-- ==================================================

-- A. Confirm app_user exists (migration 015 already applied). Expected: one row.
SELECT to_regclass('app_user') AS app_user_exists;

-- B. Confirm migration 016 has NOT already been applied. Expected: NULL.
SELECT to_regclass('auth_identity') AS auth_identity_table;

-- ==================================================
-- 016 / AUTH-02 — auth_identity (provider-neutral)
-- ==================================================

begin;

create table if not exists auth_identity (
    id uuid primary key default gen_random_uuid(),
    app_user_id uuid not null references app_user(id) on delete cascade,
    provider text not null,
    provider_subject text not null,
    provider_email text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    last_login_at timestamptz,
    constraint auth_identity_provider_subject_unique
        unique (provider, provider_subject)
);

create index if not exists idx_auth_identity_app_user
    on auth_identity(app_user_id);

create index if not exists idx_auth_identity_provider_email
    on auth_identity(provider, provider_email)
    where provider_email is not null;

commit;

-- ==================================================
-- POSTFLIGHT — read-only verification
-- ==================================================

-- Expect: auth_identity table exists, 0 rows, one unique constraint.
SELECT
    (SELECT to_regclass('auth_identity') IS NOT NULL) AS table_exists,
    (SELECT count(*) FROM auth_identity) AS identity_count;

SELECT conname
FROM pg_constraint
WHERE conrelid = 'auth_identity'::regclass
  AND conname = 'auth_identity_provider_subject_unique';
