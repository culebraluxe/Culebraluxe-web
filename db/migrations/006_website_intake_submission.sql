-- CulebraLuxe Portal
-- Website intake receipt and retry boundary
-- Migration: 006_website_intake_submission.sql

begin;

create table website_intake_submission (
    id uuid primary key,
    request_type text not null
        check (request_type in ('private_viewing', 'property_information')),
    property_id uuid not null
        references property(id) on delete restrict,
    display_name text not null
        check (char_length(display_name) between 1 and 200),
    email text not null
        check (char_length(email) between 3 and 320),
    message text
        check (message is null or char_length(message) <= 4000),
    status text not null default 'received'
        check (status in (
            'received',
            'processing',
            'resolution_required',
            'completed',
            'rejected'
        )),
    processing_started_at timestamptz,
    interaction_id uuid unique
        references interaction(id) on delete restrict,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint website_intake_interaction_state check (
        (status = 'completed' and interaction_id is not null)
        or (status <> 'completed' and interaction_id is null)
    ),
    constraint website_intake_processing_state check (
        (status = 'processing' and processing_started_at is not null)
        or (status <> 'processing' and processing_started_at is null)
    )
);

create index idx_website_intake_status_created
    on website_intake_submission(status, created_at);

commit;
