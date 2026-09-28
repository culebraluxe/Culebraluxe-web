-- CulebraLuxe Portal
-- AUTH-02 (schema portion) — Provider-Neutral Identity Mapping
-- Migration: 016_auth_identity.sql
--
-- Maps an external authentication provider subject to the canonical app_user
-- application actor. Provider-specific subjects live HERE, not on person or
-- app_user, so the canonical business tables stay provider-agnostic.
--
-- Invariants:
-- - UNIQUE(provider, provider_subject) — no ambiguous identity mapping.
-- - provider_email is informational; it is NOT a canonical lookup key.
-- - One app_user may have multiple provider identities (no unique app_user_id).
-- - No email-based runtime identity guessing, no fuzzy matching.

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
