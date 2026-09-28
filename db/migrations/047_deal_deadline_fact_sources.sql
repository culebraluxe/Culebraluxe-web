-- CulebraLuxe Portal
-- CRM-22: canonical transaction deadline fact sources (additive)
-- Migration: 047_deal_deadline_fact_sources.sql
--
-- CRM-22 provides canonical application-owned milestone dates for NON-closing
-- transaction deadlines so the generic timer/job engine can schedule and
-- recover them without inventing business dates.
--
-- Only the milestones justified by actual business use get a canonical source.
-- In a residential P&S contract (jurisdiction-neutral; Culebra PR operating
-- pattern), the inspection-period contingency and the financing-commitment
-- contingency carry their own contract dates:
--
--   deal.inspection_deadline   date   inspection-period deadline (the date by
--                                     which the buyer's inspection contingency
--                                     expires)
--   deal.financing_deadline    date   financing-commitment deadline (the date
--                                     by which the buyer must obtain financing
--                                     commitment)
--
-- closing_date (migration 001) remains the canonical closing target. No other
-- milestone gets a column: appraisal / title / tax-CRIM / funds / closing-
-- documents deadlines are process-internal obligations driven by the closing
-- date, not separate contract dates — inventing dates for them would create a
-- parallel SLA framework (architect brief rejects this). workflow_app reads
-- these columns as canonical facts (workflow_app/facts.ts); the application
-- owns legality via db/deal-deadline.setDealMilestoneDeadline
-- (deal.set_inspection_deadline / deal.set_financing_deadline).
--
-- NOT auto-executed in this story. Like migrations 031/032 (CRM-19/20 deal
-- fact columns), apply manually when CRM-22 activation SQL is approved — the
-- workflow_app/facts.ts projection selects these columns.

begin;

alter table deal
    add column inspection_deadline date;

alter table deal
    add column financing_deadline date;

commit;
