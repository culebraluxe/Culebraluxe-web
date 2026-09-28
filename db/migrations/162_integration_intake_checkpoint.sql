-- CulebraLuxe
-- Resumable mailbox-intake checkpoint.
-- Migration: 162_integration_intake_checkpoint.sql
--
-- WHY: acquisition must make mailbox SIZE irrelevant. A large mailbox can never
-- be pulled as one giant in-memory/one-shot operation, and the process may die at
-- any point. The acquisition contract is therefore:
--
--   source -> bounded page/shard -> land immediately -> checkpoint -> next page
--
-- This table is the "checkpoint" in that contract. It is deliberately a TINY
-- RESUME TOKEN, not a workflow/orchestration engine: one row per
-- (source, source_account, shard). There is no scheduler, no state machine, no
-- retry queue, no competing orchestration system - the bounded intake scripts own
-- their own loop and use this row only to answer "where did I get to?".
--
-- PROGRESSION RULE (enforced by the intake code, not by SQL): the cursor advances
-- ONLY after every record from that page has landed or replayed. A crash halfway
-- through a page leaves the previous cursor in place, so the page is simply redone
-- and the L-table replay key absorbs the duplicates.
--
-- SEPARATION OF JOBS: this is ACQUISITION only. It records no Person decision and
-- ties acquisition to nothing in CRM - promotion/reconciliation happens AFTER
-- durable landing, from the L tables, entirely independently of this row.
--
--   Gmail API   -> l_email      shards: one calendar month each (2026-08, ...)
--   Apple Mail  -> l_applemail  shards: one mailbox each (inbox, sent)
--
-- No deletes, no truncates, no "start over" cleanup is ever performed on landing
-- tables; a shard is re-run from its own first page if its saved cursor is
-- rejected, and re-landing is idempotent.

begin;

create table if not exists integration_intake_checkpoint (
    id                uuid primary key default gen_random_uuid(),
    -- the acquisition source, e.g. 'gmail' | 'applemail'
    source            text not null,
    -- the mailbox this shard belongs to, e.g. penfield33@gmail.com
    source_account    text not null,
    -- a stable identifier for the shard: a Gmail 'YYYY-MM' month, or an Apple
    -- 'mailbox:inbox' / 'mailbox:sent'. Bounded and independent of any other shard.
    shard_key         text not null,
    -- the shard's date window, when it has one (both null for cursor-only shards)
    shard_start       timestamptz,
    shard_end         timestamptz,
    -- the transport resume token: a Gmail nextPageToken, or a serialized Apple
    -- paging cursor. NULL means "start this shard from its first page".
    cursor            text,
    pages_completed   integer  not null default 0,
    records_seen      bigint   not null default 0,
    records_landed    bigint   not null default 0,
    records_replayed  bigint   not null default 0,
    errors            integer  not null default 0,
    -- 'in_progress' while the shard still has pages; 'complete' when the
    -- transport reported no further pages (nextPageToken absent).
    status            text not null default 'in_progress'
        check (status in ('in_progress', 'complete')),
    updated_at        timestamptz not null default now(),
    constraint integration_intake_checkpoint_unique
        unique (source, source_account, shard_key)
);

create index if not exists integration_intake_checkpoint_status_idx
    on integration_intake_checkpoint (source, source_account, status);

comment on table integration_intake_checkpoint is
    'Resumable bounded-intake checkpoint. One row per (source, source_account, shard). The cursor advances ONLY after every record in a page has landed or replayed. A resume token, not an orchestration engine. Acquisition only: it carries no Person decision and is not read by promotion.';

commit;
