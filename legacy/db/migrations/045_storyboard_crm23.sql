-- CulebraLuxe Portal
-- CRM-23: record the story on the Story Board (idempotent, no status change)
-- Migration: 045_storyboard_crm23.sql
--
-- CRM-23 ("macOS External Activity Observer + Durable Integration Inbox") is
-- not present on the authoritative 8/21 board (migration 022). This migration
-- records the story on the board WITHOUT fabricating a completion status: a
-- missing row is inserted as 'In Progress' (the implementation batch); an
-- existing row has ONLY its notes reconciled — status/completion are never
-- overwritten (the human-owned board stays authoritative for execution
-- control, per the Story Execution Contract).
--
-- Applied to the disposable DEV branch as part of CRM-23. Promotion to
-- production happens only through an explicit production-release task.

begin;

insert into storyboard_story
    (id, workstream, title, priority, status, notes, completion, rollup)
values (
    'CRM-23',
    'CRM',
    'macOS External Activity Observer + Durable Integration Inbox',
    'Medium-High',
    'In Progress',
    'Mac is an integration edge, not the CRM. lib/mac-observer: source-neutral ExternalActivityEvent contract + honest SourceCapability per source (contacts/calendar available; mail unproven until Mail + Full Disk Access or IMAP app credentials are configured; messages/iMessage and whatsapp unsupported — no public macOS API, no fake semantics) + MacIntegrationObserver (acquisition only; never decides tasks/deals/workflows). lib/integration-inbox: durable inbox (migration 044) with UNIQUE (source, source_account, external_event_id) dedupe, replay, claim/reclaim, bounded retry, poison/dead-letter + HumanRequired escalation, correlation + provenance references; mapper reuses the existing CRM intake stubs (calendar/email/communications/whatsapp coordinators + generic contacts identity resolution, allowCreation=false); CRM changes go through the canonical Business Command layer (interaction.record, CRM-14J seam); raw payloads are referenced, never persisted (content/provenance references only). Verified by workflow_app/tests/mac-observer-inbox.test.ts (targeted, zero Neon).',
    10,
    true
)
on conflict (id) do update
    set notes = excluded.notes,
        updated_at = now();

commit;
