-- CulebraLuxe Portal
-- CRM-14C: canonical financing fact (additive)
-- Migration: 020_deal_financing_type.sql
--
-- deal.financing_type is the canonical financing applicability fact.
--   NULL       = unknown / unresolved
--   'cash'     = transaction does not require lender financing
--   'financed' = lender financing applies
--
-- Lender participation remains responsibility/SME data only; the absence of a
-- lender participant does NOT mean cash.
--
-- NOT executed. Apply manually when CRM-14 activation SQL is approved.

begin;

alter table deal
    add column financing_type text
        check (financing_type is null or financing_type in ('cash', 'financed'));

commit;
