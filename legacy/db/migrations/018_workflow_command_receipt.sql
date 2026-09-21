-- CulebraLuxe Portal
-- CRM-14: workflow command idempotency receipt (additive)
-- Migration: 018_workflow_command_receipt.sql
--
-- The workflow engine keeps its own process_commands table; this application-
-- side receipt is the second, independent guard: the same engine commandId
-- must never duplicate a business effect. Minimal and generic — no event
-- sourcing.
--
-- NOT executed. Apply manually when CRM-14 write commands are wired.

begin;

create table workflow_command_receipt (
    command_id text primary key,

    outcome text not null,
    aggregate_id uuid,
    message text,

    created_at timestamptz not null default now()
);

commit;
