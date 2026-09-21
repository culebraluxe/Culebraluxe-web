-- CulebraLuxe Portal
-- CORE-DAILY-01 — relationship follow-up lifecycle.
--
-- Extends the EXISTING application follow-up/task model (the `task` table) into
-- a deterministic, auditable, replay-safe relationship follow-up lifecycle.
-- This is NOT a second task system: it adds lifecycle fields + states to `task`
-- and a command/receipt table so every follow-up mutation is idempotent and
-- auditable via a canonical command receipt.
--
-- Rules honored here:
--   - "Done" completes only the current relationship obligation.
--   - "Done + next touch" creates exactly one next obligation (new task row).
--   - "Snooze" never touches workflow/legal deadlines (it only sets snoozed_until).
--   - Replay/duplicate command_id never re-applies a side effect (receipt unique).

begin;

-- 1) Extend task with relationship-follow-up lifecycle fields.
alter table task
    add column if not exists snoozed_until timestamptz,
    add column if not exists outcome text,
    add column if not exists next_touch_at timestamptz,
    add column if not exists source text,
    add column if not exists reason text,
    add column if not exists recommendation_key text;

-- 2) Extend the task status set with the follow-up lifecycle states.
alter table task drop constraint if exists task_status_check;
alter table task add constraint task_status_check
    check (status in ('open', 'snoozed', 'completed', 'dismissed', 'cancelled'));

-- 3) Command/receipt table for replay-safe, auditable follow-up commands.
create table if not exists relationship_follow_up_receipt (
    id uuid primary key default gen_random_uuid(),
    command_id uuid not null unique,
    command_type text not null
        check (command_type in ('create', 'snooze', 'complete', 'dismiss', 'cancel')),
    follow_up_id uuid references task(id) on delete cascade,
    person_id uuid references person(id) on delete cascade,
    actor_user_id uuid references app_user(id) on delete set null,
    applied boolean not null default false,
    duplicate boolean not null default false,
    occurred_at timestamptz not null default now(),
    result jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    constraint relationship_follow_up_receipt_result_object
        check (jsonb_typeof(result) = 'object')
);

create index if not exists idx_follow_up_receipt_command
    on relationship_follow_up_receipt(command_id);
create index if not exists idx_follow_up_snoozed
    on task(snoozed_until) where status = 'snoozed';

commit;
