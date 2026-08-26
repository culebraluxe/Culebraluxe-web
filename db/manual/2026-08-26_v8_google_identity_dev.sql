-- CulebraLuxe Portal
-- AUTH-09E — DEV Google Identity Provisioning (ROOT) — Human-Executable Record
-- Date: 2026-08-26
--
-- Records the EXPLICIT DEV mapping of the authenticated Google ROOT subject to
-- the canonical root app_user, and grants the seeded 'owner' role so the
-- account holds portal.read. This mirrors what was applied to the DEV database
-- via scripts/provision-dev-root.ts.
--
-- NOTE: an earlier DEV auth_identity row mapped a per-login randomUUID
-- (user.id) artifact to the Lisa app_user; that value was NOT a stable Google
-- sub and is not a valid identity. The durable identity key is the STABLE
-- Google provider subject (account.providerAccountId), used below.
--
-- Identity key = provider_subject (the stable Google `sub`). provider_email is
-- informational ONLY and is left NULL — it is NEVER an identity key.
--
-- Guards / invariants:
--   - target app_user must exist, be internal, and be active
--   - the subject must not already be mapped to a DIFFERENT app_user
--   - idempotent (ON CONFLICT DO NOTHING)
--   - no app_user is created here; no role definition is changed
--   - no secrets are stored (the subject is a stable, non-secret identifier)

-- ==================================================
-- PREFLIGHT — read-only
-- ==================================================

SELECT id, display_name, email, account_type, active
FROM app_user
WHERE id = '1fc6dc61-d842-4d29-a20b-93c79e07c718';

SELECT provider, provider_subject, provider_email, app_user_id
FROM auth_identity
WHERE provider = 'google';

-- ==================================================
-- PROVISION — replace the subject below with the real captured Google sub
-- ==================================================

begin;

do $$
declare
    v_user_id uuid := '1fc6dc61-d842-4d29-a20b-93c79e07c718';
    v_subject text := '104033509608344385707';
    v_target  record;
    v_role_id uuid;
begin
    select id, account_type, active into v_target
    from app_user
    where id = v_user_id;

    if v_target.id is null then
        raise exception 'google link failed: target app_user not found';
    end if;
    if v_target.account_type <> 'internal' or v_target.active is not true then
        raise exception 'google link failed: target app_user must be internal AND active';
    end if;

    if exists (
        select 1
        from auth_identity
        where provider = 'google'
          and provider_subject = v_subject
          and app_user_id <> v_user_id
    ) then
        raise exception 'google link failed: subject already mapped to another app_user';
    end if;

    insert into auth_identity (app_user_id, provider, provider_subject, provider_email)
    values (v_user_id, 'google', v_subject, null)
    on conflict (provider, provider_subject) do nothing;

    select id into v_role_id
    from role
    where code = 'owner' and active = true;

    if v_role_id is null then
        raise exception 'owner role not found; run migration 015 first';
    end if;

    insert into app_user_role (app_user_id, role_id, assigned_by_user_id)
    values (v_user_id, v_role_id, null)
    on conflict (app_user_id, role_id) do nothing;
end;
$$;

commit;

-- ==================================================
-- POSTFLIGHT — read-only verification
-- ==================================================

SELECT
    ai.provider,
    ai.provider_subject,
    ai.app_user_id,
    u.display_name,
    u.account_type,
    u.active
FROM auth_identity ai
JOIN app_user u ON u.id = ai.app_user_id
WHERE ai.provider = 'google';

SELECT
    u.display_name,
    r.code AS role_code,
    a.code AS authority_code
FROM app_user u
JOIN app_user_role aur ON aur.app_user_id = u.id
JOIN role r ON r.id = aur.role_id AND r.active = true
LEFT JOIN role_authority ra ON ra.role_id = r.id
LEFT JOIN authority a ON a.id = ra.authority_id
WHERE u.id = '1fc6dc61-d842-4d29-a20b-93c79e07c718'
  AND r.code = 'owner';
