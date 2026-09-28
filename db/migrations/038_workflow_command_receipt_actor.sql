-- CulebraLuxe Portal
-- AUTH-05 — Sensitive Administrative Write Audit
-- Migration: 038_workflow_command_receipt_actor.sql
--
-- Additive: command receipts now record WHO performed the consequential
-- business command (offer.accept, deal.set_stage_*, deal.set_financing_type,
-- deal.set_closing_date). The receipt already carried outcome, aggregate_id,
-- message and created_at; the acting app_user is the missing audit dimension.
-- The actor is optional (engine-driven commands may have none) and is written
-- in the SAME transaction as the mutation + receipt, so it never gates the
-- mutation and reads never depend on it.

begin;

alter table workflow_command_receipt
    add column if not exists actor_app_user_id uuid references app_user(id) on delete set null;

create index if not exists idx_workflow_command_receipt_actor
    on workflow_command_receipt(actor_app_user_id);

commit;
