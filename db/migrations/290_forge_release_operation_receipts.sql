-- Preserve an observed release result separately from workflow evidence so a retry can settle it
-- without repeating an external publication, migration, or refresh operation.
create table if not exists forge_release_operation_receipt (
    command_id text primary key,
    process_instance_id uuid not null references process_instances(id) on delete cascade,
    story_id text not null references storyboard_story(id) on delete cascade,
    command_type text not null,
    result jsonb not null,
    settled boolean not null default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_forge_release_operation_receipt_process
    on forge_release_operation_receipt (process_instance_id, command_type, updated_at desc);
