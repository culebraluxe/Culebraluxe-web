-- CulebraLuxe Portal
-- CRM-14: workflow task correlation (additive)
-- Migration: 019_workflow_task_correlation.sql
--
-- The canonical CulebraLuxe `task` stays the user-facing work item; the engine
-- keeps its own runtime task state. This table is the deterministic
-- correlation key between the two. No duplicate visible tasks on retry.
--
-- NOT executed. Apply manually when CRM-14 task materialization is wired.

begin;

create table workflow_task_correlation (
    workflow_task_id text primary key,

    application_task_id uuid not null
        references task(id)
        on delete cascade,

    subject_type text not null,
    subject_id uuid not null,

    created_at timestamptz not null default now(),

    constraint workflow_task_correlation_app_unique
        unique (application_task_id)
);

create index idx_workflow_task_correlation_app
    on workflow_task_correlation (application_task_id);

commit;
