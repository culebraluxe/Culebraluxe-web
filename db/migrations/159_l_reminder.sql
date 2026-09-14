-- CulebraLuxe
-- Apple Reminders landing mirror.
-- Migration: 159_l_reminder.sql
--
-- Apple Calendar events and Apple Reminders are deliberately separate domains:
--   EKEvent    -> l_calendar  -> Schedule
--   EKReminder -> l_reminder  -> Work
--
-- Reminders are mutable task state (due date, completion, priority), so this
-- table is a current-state landing mirror rather than an append-only event log.
-- Canonical CulebraLuxe WBS rows are NOT created or mutated by this landing.

begin;

create table if not exists l_reminder (
    id                uuid primary key default gen_random_uuid(),
    source_account    text,
    source_message_id text not null,
    external_id       text,
    list_name         text,
    title             text,
    notes             text,
    starts_at         timestamptz,
    due_at            timestamptz,
    completed         boolean not null default false,
    completed_at      timestamptz,
    priority          integer,
    raw               jsonb,
    first_seen_at     timestamptz not null default now(),
    last_seen_at      timestamptz not null default now()
);

create unique index if not exists l_reminder_source_unique
    on l_reminder (coalesce(source_account, ''), source_message_id);

create index if not exists l_reminder_due_idx
    on l_reminder (due_at desc)
    where completed = false;

create index if not exists l_reminder_open_idx
    on l_reminder (completed, last_seen_at desc);

comment on table l_reminder is
    'CURRENT-STATE LANDING mirror for Apple EKReminder tasks. Separate from l_calendar events and from canonical WBS work.';

commit;
