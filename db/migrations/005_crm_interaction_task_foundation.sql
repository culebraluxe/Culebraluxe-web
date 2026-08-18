-- CulebraLuxe Portal
-- CRM interaction and task foundation
-- Migration: 005_crm_interaction_task_foundation.sql

begin;

alter table interaction
    add column event_type text;

update interaction
set event_type = channel
where event_type is null;

alter table interaction
    alter column event_type set not null,
    add column source_metadata jsonb not null default '{}'::jsonb,
    add constraint interaction_source_metadata_object
        check (jsonb_typeof(source_metadata) = 'object');

alter table interaction
    drop constraint interaction_channel_check;

alter table interaction
    add constraint interaction_channel_check
        check (channel in (
            'website',
            'email',
            'call',
            'imessage',
            'sms',
            'calendar',
            'meeting',
            'showing',
            'document',
            'manual',
            'note'
        ));

drop index if exists idx_interaction_source;

create unique index interaction_source_identity_unique
    on interaction(source_system, source_external_id)
    where source_system is not null
      and source_external_id is not null;

alter table task
    add column source_interaction_id uuid
        references interaction(id)
        on delete set null,
    add column task_kind text not null default 'human'
        check (task_kind in ('human', 'system')),
    add column priority smallint not null default 0
        check (priority >= 0);

create index idx_task_source_interaction
    on task(source_interaction_id);

create index idx_task_assignee_status_due
    on task(assigned_user_id, status, due_at);

commit;
