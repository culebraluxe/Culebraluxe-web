-- CulebraLuxe Portal
-- AUTH-01 — Break-glass Identity Link — Human-Executable (DO NOT RUN UNATTENDED)
-- Date: 2026-08-22
--
-- Maps the break-glass Credentials provider's stable subject to the configured
-- root app_user, so a recovery login resolves through the SAME canonical
-- projection as a normal provider login (auth_identity → app_user → roles).
--
-- The subject is DETERMINISTIC and DERIVED from the root app_user id:
--   provider = 'break-glass'
--   provider_subject = 'break-glass:' || app_user.id
-- auth.ts's break-glass Credentials provider returns exactly this subject, so
-- this script and the provider can never drift. Never use email as the subject.
--
-- Guards:
--   - target must exist, be internal, and be active (owner role is assigned
--     separately via 2026-08-20_v4_owner_bootstrap.sql)
--   - the derived subject must not already map to a DIFFERENT app_user
--   - idempotent (ON CONFLICT DO NOTHING)
--
-- Run AFTER the owner role bootstrap and BEFORE exercising /login/recovery.

-- ==================================================
-- PREFLIGHT — read-only
-- ==================================================

-- Candidates: active internal app_users (pick the configured root).
SELECT id, display_name, email, account_type, active
FROM app_user
WHERE account_type = 'internal' AND active = true
ORDER BY display_name;

-- Existing mappings (informational).
SELECT ai.provider, ai.provider_subject, u.display_name
FROM auth_identity ai
JOIN app_user u ON u.id = ai.app_user_id
ORDER BY ai.provider, ai.provider_subject;

-- ==================================================
-- BOOTSTRAP — replace the email with the root app_user's canonical email
-- ==================================================

begin;

do $$
declare
    v_user_id uuid;
    v_subject text;
begin
    select id into v_user_id
    from app_user
    where email = 'REPLACE_WITH_ROOT_APP_USER_EMAIL'
      and account_type = 'internal'
      and active = true;

    if v_user_id is null then
        raise exception 'break-glass link failed: no active internal app_user with that email';
    end if;

    v_subject := 'break-glass:' || v_user_id::text;

    if exists (
        select 1
        from auth_identity
        where provider = 'break-glass'
          and provider_subject = v_subject
          and app_user_id <> v_user_id
    ) then
        raise exception 'break-glass link failed: derived subject already mapped to another app_user';
    end if;

    insert into auth_identity (app_user_id, provider, provider_subject, provider_email)
    values (v_user_id, 'break-glass', v_subject, null)
    on conflict (provider, provider_subject) do nothing;
end;
$$;

commit;

-- ==================================================
-- POSTFLIGHT — read-only verification
-- ==================================================

SELECT ai.provider, ai.provider_subject, u.display_name, u.email
FROM auth_identity ai
JOIN app_user u ON u.id = ai.app_user_id
WHERE ai.provider = 'break-glass'
ORDER BY ai.provider_subject;
