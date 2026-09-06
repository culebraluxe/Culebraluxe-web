-- CulebraLuxe
-- P&S canonical round-trip: legal-entity Seller plus human representative.
-- Migration: 120_pns_seller_representative_role.sql
--
-- A Firm may be the legal SELLER while a Person signs/acts for that Firm.
-- The Person must not be mislabeled SELLER merely to keep the Contract
-- discoverable from the broker's selected Person context.

begin;

insert into role (scope, code, name, description)
values (
  'contract_person',
  'SELLER_REPRESENTATIVE',
  'Seller Representative',
  'Person authorized to act or sign for a Firm/legal-entity Seller on this Contract.'
)
on conflict (scope, code) do nothing;

insert into role_alias (role_id, scope, alias, normalized_alias)
select r.id, r.scope, a.alias, lower(regexp_replace(trim(a.alias), '\s+', ' ', 'g'))
from role r
join (values
  ('contract_person', 'SELLER_REPRESENTATIVE', 'seller representative'),
  ('contract_person', 'SELLER_REPRESENTATIVE', 'entity representative'),
  ('contract_person', 'SELLER_REPRESENTATIVE', 'seller signer')
) as a(scope, code, alias)
  on r.scope = a.scope and r.code = a.code
on conflict (scope, normalized_alias) do nothing;

commit;
