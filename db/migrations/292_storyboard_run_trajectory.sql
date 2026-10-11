-- Trajectory telemetry: what one agent turn did, summarized per Story Run and harness profile. Measurement only: nothing
-- reads this to choose the workflow. A run can have one row per profile it ran under; a repeat replaces the summary.
create table if not exists storyboard_run_trajectory (
    run_id uuid not null references storyboard_story_run(id) on delete cascade,
    harness_profile_id text not null,
    tool_call_count integer not null default 0,
    edit_call_count integer not null default 0,
    shell_call_count integer not null default 0,
    structured_call_count integer not null default 0,
    first_edit_at_ms bigint,
    last_edit_at_ms bigint,
    verification_calls_after_last_edit integer not null default 0,
    recorded_at timestamptz not null default now(),
    primary key (run_id, harness_profile_id)
);
