-- CulebraLuxe Portal
-- CMD-01 — Canonical Business Command Envelope: durable receipt chain metadata
-- Migration: 051_cmd01_command_receipt_chain.sql
--
-- Additive. The command receipt (migration 018) already proves WHICH commandId
-- executed, its outcome, aggregate and message. CMD-01 additionally records on
-- the same row:
--   actor_app_user_id  WHO issued the command (nullable; engine-driven commands
--                      carry no actor). DDL mirrors AUTH-05 migration 038
--                      EXACTLY (uuid + FK + index) so either migration applies
--                      first and the second is a no-op.
--   command_type       WHAT stable machine command type was invoked.
--   correlation_id     the correlation chain this command belonged to.
--   causation_id       the causation chain this command belonged to.
--
-- All four are written by the canonical dispatcher / receipt repository in the
-- SAME transaction as the business mutation + receipt outcome, so the receipt
-- durably answers: seen before? who issued? what type? succeeded/failed? which
-- resource? replay? which chain?

begin;

alter table workflow_command_receipt
    add column if not exists actor_app_user_id uuid references app_user(id) on delete set null;

create index if not exists idx_workflow_command_receipt_actor
    on workflow_command_receipt(actor_app_user_id);

alter table workflow_command_receipt
    add column if not exists command_type text,
    add column if not exists correlation_id text,
    add column if not exists causation_id text;

commit;
