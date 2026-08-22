-- CulebraLuxe Portal
-- ENG-07: record the Canonical DEV Lifecycle Command outcome on the Story
-- Board (notes-only; no status/completion change)
-- Migration: 050_storyboard_eng07.sql
--
-- DECISION (recorded per the architect brief): BUILD, not deferred — the
-- seams ENG-07 would wrap have landed (reconcileWorkflows; engine lease
-- primitives reclaimStaleJobs / runDueJobs from CRM-14F; the read-only
-- diagnostics snapshot; the DEV-only reset), so the command is now the single
-- boring entry point for the hardening batch's operator capabilities.
--
-- What was built (minimal by design):
--   - one canonical command: `pnpm workflow` -> scripts/workflow-cli.ts, with
--     subcommands status / reconcile / reclaim / poll / test /
--     test:persistence / reset:dev.
--   - each subcommand delegates 1:1 to an existing typed function and adds no
--     workflow logic (the CLI only parses arguments and formats output).
--   - authority/environment awareness: reset:dev refuses unless
--     APP_ENV=development (assertDevResetAllowed) and requires explicit --yes;
--     status is read-only (getWorkflowDiagnosticsSnapshot); reconcile/reclaim/
--     poll are idempotent bounded passes (the seams' contract).
--   - test / test:persistence delegate to the existing pnpm scripts (ENG-04).
--
-- Explicitly NOT built (scope exclusions honored): no daemon/cron manager, no
-- process supervisor, no custom test runner, no web UI, no monitoring/
-- alerting. Deployment/execution stays with the agent loop (ENG-06) and any
-- future Vercel Cron.
--
-- The existing seed row (migration 022, status 'Planned') has ONLY its notes
-- reconciled — status/completion are never overwritten (the human-owned board
-- stays authoritative for execution control, per the Story Execution
-- Contract). Like migration 048, this file is captured for later application;
-- it is NOT auto-executed by the ENG-07 story.

begin;

update storyboard_story
set notes = 'DECISION: BUILD (not deferred) — the seams it would wrap have landed (reconcileWorkflows; CRM-14F lease primitives reclaimStaleJobs/runDueJobs; read-only diagnostics snapshot; DEV-only reset). Canonical command added: `pnpm workflow` -> scripts/workflow-cli.ts with subcommands status (read-only diagnostics snapshot + anomaly count), reconcile (reconcileWorkflows), reclaim (reclaimStaleJobs, default batch 20), poll (one bounded runDueJobs pass, default worker workflow-cli / batch 10), test (delegates to `pnpm test`), test:persistence (delegates to `pnpm test:persistence`, ENG-04), reset:dev (assertDevResetAllowed — refuses outside APP_ENV=development — plus explicit --yes confirmation). Each subcommand delegates 1:1 to an existing typed function; the CLI adds no workflow logic. Explicitly NOT built: daemon/cron manager, process supervisor, custom test runner, web UI, monitoring/alerting — deployment/execution stays with the agent loop (ENG-06) and any future Vercel Cron. Verified by targeted dispatch unit tests (scripts/workflow-cli.test.ts) + DEV smoke tests; full regression not run per runtime policy.',
    updated_at = now()
where id = 'ENG-07';

commit;
