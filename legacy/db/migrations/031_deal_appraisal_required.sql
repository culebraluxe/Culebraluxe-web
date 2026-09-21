-- CulebraLuxe Portal
-- CRM-19: canonical appraisal applicability fact (additive)
-- Migration: 031_deal_appraisal_required.sql
--
-- deal.appraisal_required is the canonical appraisal applicability fact.
--   NULL  = unknown / unresolved (never silently treated as not-applicable)
--   true  = an appraisal is required for this transaction
--   false = no appraisal is required
--
-- Appraisal applicability is INDEPENDENT of financing (Story 123): a cash
-- deal may require an appraisal (buyer/seller request) and a financed deal may
-- not. The workflow reads it as the `appraisalApplicable` decision fact
-- (workflow_app/facts.ts); the application owns legality via
-- db/deal-appraisal.setDealAppraisalRequired (deal.set_appraisal_required,
-- routed but not referenced by any workflow command-node, mirroring
-- deal.set_financing_type).
--
-- NOT executed. Apply manually when CRM-19 activation SQL is approved (the
-- workflow_app/facts.ts projection selects this column).

begin;

alter table deal
    add column appraisal_required boolean;

commit;
