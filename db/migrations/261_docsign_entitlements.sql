-- 261 — Native document-signing entitlement catalog/grants.
--
-- The Rust action catalog lands in the same release. System-only signer-edge and
-- email-delivery authority remains code-narrowed in Casbin and is deliberately
-- not granted through role_entitlement.

begin;

insert into entitlement (code, operation_kind) values
    ('documentSign.read', 'query'),
    ('documentSign.write', 'command'),
    ('documentSign.issue', 'command'),
    ('documentSign.void', 'command'),
    ('signer.read', 'query'),
    ('signer.act', 'command'),
    ('signer.access.issue', 'command'),
    ('email.read', 'query'),
    ('email.queue', 'command'),
    ('email.deliver', 'command')
on conflict (code) do update
set operation_kind = excluded.operation_kind,
    active = true;

-- Read native signing state: all internal read-capable roles.
insert into role_entitlement (role_id, entitlement_id)
select r.id, e.id
from security_role r
cross join entitlement e
where r.account_type = 'internal'
  and r.code in (
      'root','owner',
      'business_power','business_power_user','bus_power_user','agent',
      'user','ops','viewer','internal_guest'
  )
  and e.code in ('documentSign.read','signer.read')
on conflict do nothing;

-- Draft preparation/editing is available to USER and above.
insert into role_entitlement (role_id, entitlement_id)
select r.id, e.id
from security_role r
cross join entitlement e
where r.account_type = 'internal'
  and r.code in (
      'root','owner',
      'business_power','business_power_user','bus_power_user','agent',
      'user','ops'
  )
  and e.code = 'documentSign.write'
on conflict do nothing;

-- Issue/void and delivery administration are Power User+ operations.
insert into role_entitlement (role_id, entitlement_id)
select r.id, e.id
from security_role r
cross join entitlement e
where r.account_type = 'internal'
  and r.code in (
      'root','owner',
      'business_power','business_power_user','bus_power_user','agent'
  )
  and e.code in (
      'documentSign.issue',
      'documentSign.void',
      'signer.access.issue',
      'email.read',
      'email.queue'
  )
on conflict do nothing;

-- signer.act is intentionally NOT role-granted: public signer actions are
-- admitted only through the document-sign-edge actor plus a validated recipient
-- capability. email.deliver is intentionally NOT role-granted: only the
-- email-delivery-worker actor may invoke it.

commit;
