-- CulebraLuxe Portal
-- AUTH-02 — First Owner Bootstrap — Human-Executable (DO NOT RUN UNATTENDED)
-- Date: 2026-08-20
--
-- Assigns the chosen app_user to the seeded 'owner' role. Human-controlled:
-- you MUST set the canonical email of the owner below. No guessing.
--
-- Guards:
--   - target must exist, be internal, and be active
--   - owner role must exist (migration 015 applied)
--   - idempotent (ON CONFLICT DO NOTHING)
--   - assigned_by_user_id is NULL (bootstrap semantics, no prior actor)
--
-- Run this AFTER the chosen owner's auth_identity mapping exists and BEFORE
-- Portal route protection is activated.

-- ==================================================
-- PREFLIGHT — read-only
-- ==================================================

-- Candidates: active internal app_users (pick one deterministically).
SELECT id, display_name, email, account_type, active
FROM app_user
WHERE account_type = 'internal' AND active = true
ORDER BY display_name;

-- Current owner assignments (informational — 0 means no owner yet).
SELECT
    u.display_name,
    u.email,
    aur.assigned_at
FROM app_user_role aur
JOIN app_user u ON u.id = aur.app_user_id
JOIN role r ON r.id = aur.role_id
WHERE r.code = 'owner';

-- ==================================================
-- BOOTSTRAP — replace the email below with the chosen owner's canonical email
-- ==================================================

begin;

do $$
declare
    v_user_id uuid;
    v_role_id uuid;
begin
    select id into v_user_id
    from app_user
    where email = 'REPLACE_WITH_OWNER_EMAIL'
      and account_type = 'internal'
      and active = true;

    if v_user_id is null then
        raise exception 'owner bootstrap failed: no active internal app_user with that email';
    end if;

    select id into v_role_id
    from role
    where code = 'owner';

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
    u.display_name,
    u.email,
    r.code AS role_code,
    aur.assigned_at,
    aur.assigned_by_user_id
FROM app_user_role aur
JOIN app_user u ON u.id = aur.app_user_id
JOIN role r ON r.id = aur.role_id
WHERE r.code = 'owner';
