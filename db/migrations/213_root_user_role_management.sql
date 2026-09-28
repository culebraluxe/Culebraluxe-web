-- Canonical internal role assignment and ROOT-only role administration.
begin;

-- The canonical Power User code did not exist on DEV. Keep legacy aliases for
-- existing assignments, but all new primary-role assignments target this row.
insert into security_role (code, name, account_type, description)
values (
    'business_power_user',
    'Business Power User',
    'internal',
    'Broad operational access without ROOT security administration'
)
on conflict (code) do nothing;

-- Role assignment is deliberately separate from changing a role's entitlements.
insert into entitlement (code, operation_kind)
values ('security.role.manage', 'command')
on conflict (code) do nothing;

-- A canonical Business Power User receives the existing operational catalog,
-- never either security-administration command.
insert into role_entitlement (role_id, entitlement_id)
select r.id, e.id
from security_role r
cross join entitlement e
where r.code = 'business_power_user'
  and r.account_type = 'internal'
  and r.active = true
  and e.active = true
  and e.code not in ('security.entitlement.manage', 'security.role.manage')
on conflict do nothing;

-- Literal ROOT alone may assign primary security roles.
insert into role_entitlement (role_id, entitlement_id)
select r.id, e.id
from security_role r
join entitlement e on e.code = 'security.role.manage'
where r.code = 'root'
  and r.account_type = 'internal'
  and r.active = true
on conflict do nothing;

commit;
