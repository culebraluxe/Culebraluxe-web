-- Correct migration 211: entitlement management is ROOT-only, not owner-equivalent.
begin;

delete from role_entitlement re
using security_role r, entitlement e
where re.role_id = r.id
  and re.entitlement_id = e.id
  and r.code = 'owner'
  and e.code = 'security.entitlement.manage';

commit;
