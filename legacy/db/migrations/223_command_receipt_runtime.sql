-- CulebraLuxe
-- 223_command_receipt_runtime.sql
--
-- The Rust canonical command runtime is now a real consumer of the richer
-- command proof contract. Keep the original claim-first command_id primary key
-- and extend the row additively with the identity/correlation/result facts
-- required for durable replay, diagnostics and external harnesses.

begin;

alter table workflow_command_receipt
    add column if not exists command_type text,
    add column if not exists correlation_id text,
    add column if not exists causation_id text,
    add column if not exists aggregate_type text,
    add column if not exists requested_at timestamptz,
    add column if not exists result_payload jsonb,
    add column if not exists error_code text,
    add column if not exists error_message text,
    add column if not exists updated_at timestamptz not null default now();

create index if not exists idx_workflow_command_receipt_type_created
    on workflow_command_receipt(command_type, created_at desc);

create index if not exists idx_workflow_command_receipt_correlation
    on workflow_command_receipt(correlation_id)
    where correlation_id is not null;

commit;
