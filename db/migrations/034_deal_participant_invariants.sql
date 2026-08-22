-- CulebraLuxe Portal
-- CRM-13: deal_participant canonical invariants (additive)
-- Migration: 034_deal_participant_invariants.sql
--
-- Makes deal_participant THE canonical participant model by enforcing the
-- application invariants at the DB layer:
--
--   1. At most ONE active structural participant (client/owner/seller) per
--      deal — partial unique index on (deal_id, role).
--   2. At most ONE active long-tail participant (role='other') per role_label
--      per deal — partial unique index on (deal_id, lower(role_label)). The
--      SME long tail (lender, inspector, appraiser, notario, title, ...) is
--      expressed as role='other' + role_label; new roles are curated labels,
--      never schema migrations.
--
-- Reconciliation first: any deal whose legacy FK (deal.client_person_id,
-- deal.owner_user_id, property.seller_person_id) has no matching ACTIVE
-- participant row gets one backfilled now, so the read projections that move
-- to deal_participant never diverge from the denormalized legacy FKs. The
-- legacy FKs remain (not dropped); participants are the read source.
--
-- Applied to the disposable DEV branch as part of CRM-13.

begin;

-- ---------------------------------------------------------------------------
-- 1. Reconcile legacy FKs -> active participant rows (one-time)
-- ---------------------------------------------------------------------------

insert into deal_participant (deal_id, person_id, role, started_at, active)
select d.id, d.client_person_id, 'client', d.created_at, true
from deal d
where d.client_person_id is not null
  and not exists (
    select 1 from deal_participant dp
    where dp.deal_id = d.id
      and dp.role = 'client'
      and dp.active
  );

insert into deal_participant (deal_id, user_id, role, started_at, active)
select d.id, d.owner_user_id, 'owner', d.created_at, true
from deal d
where d.owner_user_id is not null
  and not exists (
    select 1 from deal_participant dp
    where dp.deal_id = d.id
      and dp.role = 'owner'
      and dp.active
  );

insert into deal_participant (deal_id, person_id, role, started_at, active)
select d.id, p.seller_person_id, 'seller', d.created_at, true
from deal d
join property p
  on p.id = d.property_id
where p.seller_person_id is not null
  and not exists (
    select 1 from deal_participant dp
    where dp.deal_id = d.id
      and dp.role = 'seller'
      and dp.active
  );

-- ---------------------------------------------------------------------------
-- 2. Invariants
-- ---------------------------------------------------------------------------

-- One ACTIVE structural participant (client/owner/seller) per deal.
create unique index uq_deal_participant_active_structural_role
    on deal_participant (deal_id, role)
    where role in ('client', 'owner', 'seller') and active;

-- One ACTIVE role='other' participant per role_label per deal
-- (case-insensitive; the label is the SME long-tail vocabulary).
create unique index uq_deal_participant_active_other_label
    on deal_participant (deal_id, lower(role_label))
    where role = 'other' and active and role_label is not null;

commit;
