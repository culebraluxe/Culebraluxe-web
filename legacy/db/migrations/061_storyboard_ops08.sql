-- CulebraLuxe Portal
-- OPS-08: record the Story Board Batch / Next Work Selection outcome on the
-- Story Board (notes-only; no status/completion change)
-- Migration: 061_storyboard_ops08.sql
--
-- The 8/21 authoritative board (migration 022) describes OPS-08 as Planned/10
-- ("Add bounded work-selection capability such as Next 20 without building
-- Jira."). This run delivered that capability as a PURE, deterministic
-- projection over the authoritative stored stories — no assignments, no
-- sprints, no new tables, no schema change:
--
--   lib/storyboard-data.ts gains the Next Work selection contract
--   (selectNextWork): eligibility = rollup=true AND status in { Planned,
--   Ready, In Progress, Partial } AND every board story referenced in the
--   free-text `dependencies` is Complete (references to stories the board
--   does not know are unverifiable and never block); ordering = batch
--   ascending (unbatched last) → ENG-16 priority ladder → planned start
--   (earliest first, unplanned last) → id; capped at NEXT_WORK_DEFAULT_LIMIT
--   (20, max 50) with truncated/totalEligible/totalBlockedByDependency
--   reported. The board page (/portal/storyboard) renders the derived
--   "Next Work — Next 20" section; the existing `batch` field (migration
--   021) is the batch axis and remains a human-editable board field.
--
-- Like 056/057/058/059, this reconciles ONLY the note — status ('Planned')
-- and completion (10) are intentionally preserved; the human-owned board
-- stays authoritative for execution control, per the Story Execution
-- Contract.
--
-- Applied to the disposable DEV branch as part of OPS-08.

begin;

update storyboard_story
set notes = 'Bounded work-selection capability delivered (OPS-08 run): a PURE, deterministic "Next 20" projection over the authoritative board — no Jira, no assignments, no sprints, no schema change. lib/storyboard-data.ts gains selectNextWork: eligible = rollup=true and status in {Planned, Ready, In Progress, Partial} and every board story referenced in the free-text dependencies field is Complete (unverifiable references never block); ordered batch ascending (unbatched last) → ENG-16 priority ladder → planned start (earliest first, unplanned last) → id; capped at 20 (max 50) with totalEligible / totalBlockedByDependency / truncated surfaced. /portal/storyboard renders the derived "Next Work — Next 20" section (batch, priority, status, workstream per entry); the batch axis reuses the existing storyboard_story.batch field (migration 021), still human-editable. Verified (SCOPED policy): new workflow_app/tests/next-work.test.ts 16/16 (priority ladder, eligibility incl. rollup=false exclusion, dependency gate incl. unknown-reference and case-insensitivity, batch-before-priority ordering, planned-start and id tiebreaks, cap clamping 1..50, truncation, determinism + input immutability, empty board) plus directly adjacent storyboard-filter + storyboard-rollup + storyboard seams 42/42; tsc clean; next build --webpack passed; git diff --check clean. Status/completion are the human board decision.',
    updated_at = now()
where id = 'OPS-08'
  and status = 'Planned'
  and completion = 10;

commit;
