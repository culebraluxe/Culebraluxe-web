-- Explicit per-action entitlements. Migrate before starting a server that reads these grants.
begin;
create table entitlement (
    id uuid primary key default gen_random_uuid(),
    code text not null unique,
    operation_kind text not null check (operation_kind in ('query', 'command')),
    active boolean not null default true
);
create table role_entitlement (
    role_id uuid not null references security_role(id) on delete cascade,
    entitlement_id uuid not null references entitlement(id) on delete cascade,
    primary key (role_id, entitlement_id)
);
insert into entitlement (code, operation_kind) values
    ('accounting.read', 'query'),
    ('accounting.write', 'command'),
    ('calendar.read', 'query'),
    ('calendar.write', 'command'),
    ('cockpit.read', 'query'),
    ('comms.read', 'query'),
    ('contract.execute', 'command'),
    ('contract.read', 'query'),
    ('contract.write', 'command'),
    ('crm.write', 'command'),
    ('deal.read', 'query'),
    ('deal.write', 'command'),
    ('firm.read', 'query'),
    ('firm.write', 'command'),
    ('form.read', 'query'),
    ('form.write', 'command'),
    ('person.read', 'query'),
    ('person.write', 'command'),
    ('portal.read', 'query'),
    ('project.read', 'query'),
    ('project.write', 'command'),
    ('property.read', 'query'),
    ('property.write', 'command'),
    ('security.principal.read', 'query'),
    ('showing.read', 'query'),
    ('showing.write', 'command'),
    ('signature.read', 'query'),
    ('signature.write', 'command'),
    ('vault.issue', 'command'),
    ('vault.read', 'query'),
    ('vault.write', 'command'),
    ('wbs.read', 'query'),
    ('wbs.write', 'command')
on conflict (code) do nothing;

-- The external guest has no portal grants. Only the internal guest may read portal data.
insert into security_role (code, name, account_type, description) values
    ('internal_guest', 'Internal Guest', 'internal', 'Read only portal access'),
    ('user', 'User', 'internal', 'Read and selected edits')
on conflict (code) do nothing;

-- Existing Next.js navigation still reads coarse authorities; bridge portal entry
-- while the screen controls move to the per-action entitlement snapshot.
insert into role_authority (role_id, authority_id)
select r.id, a.id
from security_role r cross join authority a
where r.code in ('internal_guest', 'user')
  and a.code in ('portal.read', 'deal.read', 'settings.read')
on conflict do nothing;

insert into role_entitlement (role_id, entitlement_id)
select r.id, e.id
from security_role r cross join entitlement e
where (r.code in ('root', 'owner', 'business_power', 'business_power_user', 'bus_power_user', 'agent', 'internal_guest', 'user', 'ops', 'viewer')
       and e.code in ('accounting.read','accounting.write','calendar.read','calendar.write','cockpit.read','comms.read','contract.execute','contract.read','contract.write','crm.write','deal.read','deal.write','firm.read','firm.write','form.read','form.write','person.read','person.write','portal.read','project.read','project.write','property.read','property.write','security.principal.read','showing.read','showing.write','signature.read','signature.write','vault.issue','vault.read','vault.write','wbs.read','wbs.write'))
  and (r.code not in ('internal_guest', 'user', 'ops', 'viewer') or e.operation_kind = 'query'
       or (r.code in ('user', 'ops') and e.code in ('person.write', 'showing.write', 'calendar.write', 'form.write')))
  and r.account_type = 'internal'
on conflict do nothing;
commit;
