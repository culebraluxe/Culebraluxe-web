-- Durable, resumable Forge learning scans and logical finding identities.
--
-- A scan is pinned to one commit. The pending paths and observations survive
-- process/worker restarts; the local .forge-context file is only a cache.
create table if not exists forge_learn_scan_state (
    repository_key text primary key,
    cursor_revision text,
    active_revision text,
    pending_files text[] not null default '{}',
    pending_observations jsonb not null default '[]'::jsonb,
    updated_at timestamptz not null default now(),
    constraint forge_learn_scan_pending_observations_array
        check (jsonb_typeof(pending_observations) = 'array')
);

comment on table forge_learn_scan_state is
    'Pinned source revision, remaining changed paths, and unfiled observations for resumable Forge learning scans.';

create table if not exists forge_learn_finding (
    repository_key text not null,
    finding_key text not null,
    occurrence integer not null default 0 check (occurrence >= 0),
    last_story_id text references storyboard_story(id) on delete set null,
    last_seen_revision text not null,
    last_seen_at timestamptz not null default now(),
    primary key (repository_key, finding_key)
);

comment on table forge_learn_finding is
    'Stable logical finding identity and recurrence pointer; open-work truth remains in story, queue, and batch tables.';
