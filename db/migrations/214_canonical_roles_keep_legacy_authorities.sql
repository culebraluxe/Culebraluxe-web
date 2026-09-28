-- 214 — canonical security roles must keep the authorities of the legacy roles they replace.
--
-- WHY. The role-administration screen assigns CANONICAL roles (internal_guest, user, business_power_user, owner,
-- root). Authorities are the OLDER permission model, and they are still enforced: `app/portal/layout.tsx` requires
-- `portal.read` to enter the portal at all. So a role with no authorities is a role that cannot reach the portal.
--
-- MEASURED IN DEV, 2026-09-24: `business_power_user` — the canonical replacement for the legacy
-- `business_power`/`agent` codes — carried ZERO authorities while `business_power` carried six (crm.write,
-- deal.read, deal.write, listing.write, portal.read, settings.read). Moving a user onto the canonical role
-- therefore removed their portal access, which is a lockout and not a downgrade. SECURITY-CORE-01 requires the
-- legacy authority metadata to be preserved for compatibility; this restores that.
--
-- WHAT IT DOES: for each canonical role, grants the union of authorities held by the legacy roles it replaces
-- (the role-compatibility table: business_power + business_power_user + bus_power_user + agent → business_power_user;
-- user + ops + viewer → user; internal_guest + guest + client → internal_guest). Purely additive and idempotent:
-- it never revokes an authority, and re-running it repairs drift. The legacy roles stay, untouched, so anything
-- still reading them keeps working.
with groups(canonical_code, legacy_code) as (
  values
    ('business_power_user', 'business_power'),
    ('business_power_user', 'bus_power_user'),
    ('business_power_user', 'agent'),
    ('user', 'user'),
    ('user', 'ops'),
    ('user', 'viewer'),
    ('internal_guest', 'internal_guest'),
    ('internal_guest', 'guest'),
    ('internal_guest', 'client'),
    ('owner', 'owner'),
    ('root', 'root')
),
pairs as (
  select distinct canonical.id as role_id, held.authority_id
    from groups g
    join security_role canonical on canonical.code = g.canonical_code and canonical.account_type = 'internal'
    join security_role legacy on legacy.code = g.legacy_code and legacy.account_type = 'internal'
    join role_authority held on held.role_id = legacy.id
)
insert into role_authority (role_id, authority_id)
select role_id, authority_id from pairs
on conflict do nothing;
