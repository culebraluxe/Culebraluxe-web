-- ENT-01 — service authorization policies (policy-as-data).
--
-- Enforced by AuthorizationService. The code default rules remain a safety
-- floor; DB rows override by id and add new rules without a code release.
-- idempotent: safe to re-run.

create table if not exists service_authorization_policy (
  id text primary key,
  domain text not null,
  action text,
  operation text,
  kind text,
  min_level text not null,
  note text,
  created_at timestamptz not null default now(),
  constraint service_authorization_policy_level_check
    check (min_level in ('ROOT', 'BUSINESS_POWER_USER', 'USER', 'GUEST'))
);

insert into service_authorization_policy (id, domain, action, operation, kind, min_level, note)
values (
  'rule:contract.execute',
  'contract',
  null,
  'contract.execute',
  'command',
  'BUSINESS_POWER_USER',
  'Executing a Contract is high-value and near-irreversible.'
)
on conflict (id) do nothing;
