-- CulebraLuxe Portal
-- AUTH-09E — V1 ROOT / BUSINESS_POWER role + tech.access entitlement (schema/seed)
-- Migration: 083_role_tech_entitlement.sql
--
-- Adds the `tech.access` authority and the canonical V1 role split:
--   root            -> full application access + tech.access (TECH visible/allowed)
--   business_power  -> full normal brokerage access, NO tech.access (TECH hidden/denied)
--   ops / guest     -> seeded for forward-compat only (no users assigned yet)
--
-- Reuses the existing canonical tables (role, authority, role_authority). No new
-- tables, no email-based identity, no second authorization system.

begin;

insert into authority (code, name, description) values
    ('tech.access', 'Tech platform access', 'Access engineering and platform surfaces (TECH operating world)')
on conflict (code) do nothing;

insert into role (code, name, account_type, description) values
    ('root', 'ROOT', 'internal', 'Brokerage principal with full access including TECH'),
    ('business_power', 'Business Power', 'internal', 'Full normal brokerage capability; no TECH access'),
    ('ops', 'Ops', 'internal', 'Operational subset (future)'),
    ('guest', 'Guest', 'external', 'Read-only guest subset (future)')
on conflict (code) do nothing;

insert into role_authority (role_id, authority_id)
select r.id, a.id
from role r
cross join authority a
where (r.code, a.code) in (
    ('root', 'portal.read'),
    ('root', 'crm.write'),
    ('root', 'listing.write'),
    ('root', 'deal.read'),
    ('root', 'deal.write'),
    ('root', 'settings.read'),
    ('root', 'settings.manage'),
    ('root', 'tech.access'),
    ('business_power', 'portal.read'),
    ('business_power', 'crm.write'),
    ('business_power', 'listing.write'),
    ('business_power', 'deal.read'),
    ('business_power', 'deal.write'),
    ('business_power', 'settings.read')
)
on conflict (role_id, authority_id) do nothing;

commit;
