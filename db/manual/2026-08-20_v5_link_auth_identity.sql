-- CulebraLuxe Portal
-- AUTH-02 — Link Provider Identity → app_user — Human-Executable (DO NOT RUN UNATTENDED)
-- Date: 2026-08-20
--
-- Maps (provider, provider_subject) to a chosen app_user. Human-controlled.
-- Replace the placeholders in the BOOTSTRAP section below.
--
-- Guards:
--   - target app_user must exist and be active
--   - subject must not already be mapped to a DIFFERENT app_user
--   - provider_email is informational only (never an identity key)
--   - idempotent (ON CONFLICT DO NOTHING)

-- ==================================================
-- PREFLIGHT — read-only
-- ==================================================

-- Candidates: active internal app_users.
SELECT id, display_name, email, account_type, active
FROM app_user
WHERE account_type = 'internal' AND active = true
ORDER BY display_name;

-- Existing mappings (informational).
SELECT ai.provider, ai.provider_subject, ai.provider_email, u.display_name
FROM auth_identity ai
JOIN app_user u ON u.id = ai.app_user_id
ORDER BY ai.provider, ai.provider_subject;

-- ==================================================
-- BOOTSTRAP — replace placeholders with real values
-- ==================================================

begin;

do $$
declare
    v_user_id uuid;
    v_provider text := 'REPLACE_WITH_PROVIDER';         -- e.g. 'google'
    v_subject text := 'REPLACE_WITH_PROVIDER_SUBJECT';  -- the provider's stable sub
    v_email   text := 'REPLACE_WITH_PROVIDER_EMAIL';    -- informational only (nullable)
begin
    select id into v_user_id
    from app_user
    where email = 'REPLACE_WITH_APP_USER_EMAIL'
      and active = true;

    if v_user_id is null then
        raise exception 'link failed: no active app_user with that email';
    end if;

    if exists (
        select 1
        from auth_identity
        where provider = v_provider
          and provider_subject = v_subject
          and app_user_id <> v_user_id
    ) then
        raise exception 'link failed: subject already mapped to another app_user';
    end if;

    insert into auth_identity (app_user_id, provider, provider_subject, provider_email)
    values (v_user_id, v_provider, v_subject, v_email)
    on conflict (provider, provider_subject) do nothing;
end;
$$;

commit;

-- ==================================================
-- POSTFLIGHT — read-only verification
-- ==================================================

SELECT ai.provider, ai.provider_subject, ai.provider_email, u.display_name
FROM auth_identity ai
JOIN app_user u ON u.id = ai.app_user_id
ORDER BY ai.provider, ai.provider_subject;
