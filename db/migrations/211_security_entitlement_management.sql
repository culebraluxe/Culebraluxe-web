-- ROOT alone may change role grants through the Security service.
begin;
insert into entitlement (code, operation_kind)
values ('security.entitlement.manage', 'command')
on conflict (code) do nothing;

insert into role_entitlement (role_id, entitlement_id)
select r.id, e.id
from security_role r cross join entitlement e
where r.code in ('root', 'owner') and r.account_type = 'internal'
  and e.code = 'security.entitlement.manage'
on conflict do nothing;
commit;
