-- ===========================================================================
-- WORKFLOW-TRACE — business-context id columns as TEXT (evidence, not FK).
--
-- The flight-recorder trace must accept arbitrary business identifiers (UUIDs
-- today, external/non-UUID identifiers where they occur) and stay durable even
-- if an operational object later changes. Storing these as uuid would make the
-- recorder choke on non-UUID identifiers (Postgres 22P02) and drop evidence,
-- and would turn the trace into an FK-heavy dependency monster the design
-- explicitly forbids. TEXT keeps the recorder robust and observer-only.
-- ===========================================================================

alter table public.workflow_execution_trace_event
    alter column deal_id drop default,
    alter column deal_id type text,
    alter column person_id type text,
    alter column property_id type text,
    alter column transaction_document_id type text;

alter table public.workflow_execution_trace_event
    alter column deal_id set default null,
    alter column person_id set default null,
    alter column property_id set default null,
    alter column transaction_document_id set default null;
