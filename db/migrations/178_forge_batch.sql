-- 178_forge_batch.sql
--
-- ENGINE BATCH BECOMES A REAL THING (a batch you can name, schedule and look back at).
--
-- Migration 177 made staging work by reusing the story status column: `Batched` means "chosen for the
-- next group, not dispatched". That is real persistence and it does fire events on command (pressing
-- Send writes `Ready` per story, and `agent_work_item_dispatch()` creates one work item each), but it
-- is a STATUS, so there is no batch: no name, no fired-at, no history, and - the reason the captain
-- asked for this - NO WAY TO SAY "not now, at 2am".
--
-- His words, 2026-09-14: "i assumed that if there was the engine batch that would be a table in neon
-- that could pop off events when i said RUN batch and the Engine Run Q was the thing next to it for
-- real time". That is this table.
--
-- WHAT IT BUYS:
--   1. a durable batch record (who, when, what, and how it ended);
--   2. `scheduled_for` - a night run that fires itself when the clock passes it, without anyone
--      awake to press a button (the unattended worker fires due batches at the top of every pass);
--   3. batch HISTORY for the Cockpit, instead of the current state being the only evidence that
--      anything was ever batched.
--
-- Non-destructive: two new tables, no existing row or column touched. `Batched` remains the staging
-- status the board reads, so nothing about the sorter changes.

create table if not exists forge_batch (
    id uuid primary key default gen_random_uuid(),
    label text,
    -- Staged   = built, not fired and not scheduled (the Send button fires it now)
    -- Scheduled = waiting for `scheduled_for` (the night run)
    -- Fired     = dispatched; members carry their own state in forge_batch_item
    -- Cancelled = deliberately abandoned before firing
    status text not null default 'Staged',
    scheduled_for timestamptz,
    fired_at timestamptz,
    created_at timestamptz not null default now(),
    created_by uuid,
    note text,
    constraint forge_batch_status_check
        check (status = any (array['Staged'::text, 'Scheduled'::text, 'Fired'::text, 'Cancelled'::text]))
);

create table if not exists forge_batch_item (
    batch_id uuid not null references forge_batch(id) on delete cascade,
    story_id text not null references storyboard_story(id) on delete cascade,
    -- Staged = in the batch; Queued = dispatched to the engine (status Ready); Skipped = refused
    state text not null default 'Staged',
    queued_at timestamptz,
    error_text text,
    primary key (batch_id, story_id),
    constraint forge_batch_item_state_check
        check (state = any (array['Staged'::text, 'Queued'::text, 'Skipped'::text]))
);

-- The unattended worker asks this question every pass: "is any batch due?"
create index if not exists forge_batch_due_idx on forge_batch (status, scheduled_for);
-- "Which batches has this story been in?" (history on a story, and overlap checks)
create index if not exists forge_batch_item_story_idx on forge_batch_item (story_id);
