-- CulebraLuxe Portal
-- CRM-20: canonical lender clear-to-close fact (additive)
-- Migration: 032_deal_lender_clear_to_close.sql
--
-- deal.lender_clear_to_close is the canonical lender clear-to-close fact.
--   NULL  = unresolved (financed deal: not yet recorded; cash deal: not applicable)
--   true  = the lender has cleared the transaction to close
--   false = the lender has NOT cleared the transaction
--
-- Consumed only for financed deals (financingApplicable == true): the workflow
-- closing-readiness gate requires lenderClearToClose == true before readiness
-- can succeed; null routes to an explicit resolution task, false is a pending
-- state that blocks readiness. Cash/non-financed deals are unaffected. The
-- workflow reads it as the `lenderClearToClose` decision fact
-- (workflow_app/facts.ts); the application owns legality via
-- db/deal-lender-clearance.setDealLenderClearToClose
-- (deal.set_lender_clear_to_close, routed but not referenced by any workflow
-- command-node, mirroring deal.set_financing_type / deal.set_appraisal_required).
-- Lender provider behavior is never modeled inside the workflow engine.
--
-- NOT executed. Apply manually when CRM-20 activation SQL is approved (the
-- workflow_app/facts.ts projection selects this column).

begin;

alter table deal
    add column lender_clear_to_close boolean;

commit;
