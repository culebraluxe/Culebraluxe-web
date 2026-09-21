-- ===========================================================================
-- WORKFLOW RUNTIME — flight-recorder trace event table.
--
-- Durable, immutable, observer-only execution trace. Records architecturally
-- meaningful boundaries (command, domain event, workflow node/transition, task,
-- timer/job, document, signature, failure/recovery) so the Workflow Runtime
-- Inspector can answer "what actually happened, why, and what happened next"
-- against the design-time Business Process Mapper topology.
--
-- This is OPERATIONAL EVIDENCE, NOT business state. Recorder writes must NEVER
-- gate a business transaction. All foreign/context fields are nullable because
-- not every trace event belongs to every domain object; the table is deliberately
-- NOT an FK-heavy dependency monster. Trace evidence stays durable even if an
-- operational object later changes.
--
-- Replay safety: a deterministic (source_system, source_event_id) partial-unique
-- index is the backstop so a replayed upstream receipt/logical event records ONCE
-- without suppressing legitimate distinct activity (distinct events carry
-- distinct source_event_id).
-- ===========================================================================

create table if not exists public.workflow_execution_trace_event (
    id uuid primary key default gen_random_uuid(),

    -- Identity / correlation / causation
    trace_id text,
    correlation_id text,
    causation_id text,

    -- Business context (nullable; not every event belongs to every object)
    deal_id uuid,
    person_id uuid,
    property_id uuid,
    transaction_document_id uuid,

    -- Workflow context
    workflow_instance_id text,
    workflow_definition_key text,
    workflow_definition_version int,
    workflow_node_id text,
    workflow_transition_id text,

    -- Event facts
    event_type text not null,
    system text not null,
    occurred_at timestamptz not null,
    completed_at timestamptz,
    duration_ms int,
    outcome text,

    -- Related ids
    command_id text,
    domain_event_id text,
    task_id text,
    timer_job_id text,
    signature_request_id text,
    external_reference text,

    -- Bounded detail
    summary text,
    metadata jsonb,

    -- Replay / source identity
    source_system text,
    source_event_id text,

    recorded_at timestamptz not null default now()
);

-- Retrieval by the Runtime Inspector entry points.
create index if not exists wfx_trace_workflow_instance_idx
    on public.workflow_execution_trace_event (workflow_instance_id, occurred_at desc);

create index if not exists wfx_trace_deal_idx
    on public.workflow_execution_trace_event (deal_id, occurred_at desc);

create index if not exists wfx_trace_trace_idx
    on public.workflow_execution_trace_event (trace_id, occurred_at desc);

create index if not exists wfx_trace_correlation_idx
    on public.workflow_execution_trace_event (correlation_id, occurred_at desc);

create index if not exists wfx_trace_occurred_idx
    on public.workflow_execution_trace_event (occurred_at desc);

-- Replay backstop: one logical event per (source_system, source_event_id).
create unique index if not exists wfx_trace_source_identity_uq
    on public.workflow_execution_trace_event (source_system, source_event_id)
    where source_event_id is not null;
