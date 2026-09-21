-- CulebraLuxe Portal
-- CRM-22: record the story on the Story Board (idempotent, no status change)
-- Migration: 048_storyboard_crm22.sql
--
-- CRM-22 ("Transaction Deadline Fact Sources") is not present on the
-- authoritative 8/21 board (migration 022). This migration records the story
-- on the board WITHOUT fabricating a completion status: a missing row is
-- inserted as 'In Progress' (the implementation batch); an existing row has
-- ONLY its notes reconciled — status/completion are never overwritten (the
-- human-owned board stays authoritative for execution control, per the Story
-- Execution Contract).
--
-- Like migration 047 (the deal deadline columns), this file is captured for
-- later application; it is NOT auto-executed by the CRM-22 story.

begin;

insert into storyboard_story
    (id, workstream, title, priority, status, notes, completion, rollup)
values (
    'CRM-22',
    'CRM',
    'Transaction Deadline Fact Sources',
    'Medium-High',
    'In Progress',
    'Application/domain owns canonical milestone dates; workflow_app projects them into timer semantics; workflow_engine persists/fires/reschedules/reclaims generic timer jobs. Canonical sources added for the milestones justified by actual business use: deal.inspection_deadline (P&S inspection-period contingency) and deal.financing_deadline (P&S financing-commitment contingency), set via canonical commands deal.set_inspection_deadline / deal.set_financing_deadline (db/deal-deadline.ts, claim-first receipt). workflow_app/facts.ts projects inspectionDeadline/financingDeadline + Scheduled facts; deadlines.ts maps inspection/financing/closing to canonical sources and leaves appraisal/title/tax-CRIM/funds/closing-documents unresolved (no contract date — no artificial dates, no parallel SLA). RE_supermodel-v1.xml adds two OPTIONAL deadline-monitor fork branches (applicability + scheduled gates, timer due-at-variable, escalation task, amend command-node, re-arm loop); the join skips a still-active monitor and cancels its pending timer; engine lease reclaim (CRM-14F) and terminal-cancellation cleanup apply unchanged. Generic deterministic reschedule seam workflow_app/deadline-timer.ts (node-id-scoped; closing-timer.ts delegates). Verified by targeted unit tests (deadline-timer, deal-deadline, deadline-policy, command-inventory, re-supermodel M1-M5) + adjacent engine timer/lease tests; full regression not run per runtime policy.',
    15,
    true
)
on conflict (id) do update
    set notes = excluded.notes,
        updated_at = now();

commit;
