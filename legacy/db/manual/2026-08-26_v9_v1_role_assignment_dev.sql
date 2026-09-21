-- CulebraLuxe Portal
-- AUTH-09E — V1 ROOT / BUSINESS_POWER role assignment (DEV) — Human-Executable Record
-- Date: 2026-08-26
--
-- Assigns the canonical DEV roles after migration 083_role_tech_entitlement.sql:
--   ROOT app_user (culebraluxe@gmail.com) -> root role (full access + tech.access)
--   Lisa app_user                          -> business_power (business, no tech)
-- The redundant legacy `owner` assignment is removed so effective authority is
-- exact. Idempotent. No app_user is created here. email is provisioning metadata
-- only, never a runtime identity key.

begin;

delete from app_user_role
where (app_user_id, role_id) in (
  select au.id, r.id
  from app_user au, role r
  where r.code = 'owner'
    and au.id in (
      '1fc6dc61-d842-4d29-a20b-93c79e07c718',
      'aa06d089-162c-4bef-84ec-a76ee38cc8ad'
    )
);

insert into app_user_role (app_user_id, role_id, assigned_by_user_id)
select au.id, r.id, null
from app_user au, role r
where (au.id, r.code) in (
  ('1fc6dc61-d842-4d29-a20b-93c79e07c718', 'root'),
  ('aa06d089-162c-4bef-84ec-a76ee38cc8ad', 'business_power')
)
on conflict (app_user_id, role_id) do nothing;

commit;

-- POSTFLIGHT (read-only)
SELECT u.display_name, r.code AS role
FROM app_user u
JOIN app_user_role aur ON aur.app_user_id = u.id
JOIN role r ON r.id = aur.role_id
WHERE u.id IN ('1fc6dc61-d842-4d29-a20b-93c79e07c718', 'aa06d089-162c-4bef-84ec-a76ee38cc8ad')
ORDER BY u.display_name, r.code;
