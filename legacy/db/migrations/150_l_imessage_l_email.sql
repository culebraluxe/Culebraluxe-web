-- CulebraLuxe
-- LANDING tables for the two side feeds: iMessage and Google email.
-- Migration: 150_l_imessage_l_email.sql
--
-- PATTERN (captain, 2026-09-10): every feed lands in an L table that mirrors its
-- real table 1-to-1-ish, with light cleaning and NO judgment. Feeds never merge;
-- promotion into the canonical tables happens once, later, by identity.
--
-- iMessage and Google mail currently write straight into canonical tables, which
-- made them inconsistent with Apple Contacts (which lands in l_person*). These two
-- tables give them the same shape:
--
--   iMessage      -> l_imessage -> (later) interaction
--   Google email  -> l_email    -> (later) interaction
--
-- Landing rules: permissive columns, replay-safe on (source_account, source_id),
-- a raw jsonb for anything we do not model yet, and NO foreign keys to Person (a
-- feed row must be storable even when identity resolution would fail).
-- Non-destructive and additive.

begin;

create table if not exists l_imessage (
    id                uuid primary key default gen_random_uuid(),
    source_account    text,
    -- Apple's stable message GUID (source_external_id) — the replay key.
    source_message_id text not null,
    conversation_id   text,
    -- phone/email of the other party, as exported (normalized later, not here)
    handle            text,
    direction         text,
    service           text,
    sent_at           timestamptz,
    text_content      text,
    raw               jsonb,
    ingested_at       timestamptz not null default now()
);

create unique index if not exists l_imessage_source_unique
    on l_imessage (coalesce(source_account, ''), source_message_id);

create index if not exists l_imessage_handle_idx on l_imessage (handle);
create index if not exists l_imessage_sent_idx on l_imessage (sent_at desc);

comment on table l_imessage is
    'LANDING for Apple iMessage. One row per message, replay-safe on (source_account, source_message_id). No judgment, no merging; promotion into canonical happens later by identity.';

create table if not exists l_email (
    id                uuid primary key default gen_random_uuid(),
    source_account    text,
    -- RFC Message-ID, or the provider id when that is all we have.
    source_message_id text not null,
    thread_id         text,
    from_address      text,
    to_address        text,
    subject           text,
    sent_at           timestamptz,
    body_preview      text,
    labels            text[],
    raw               jsonb,
    ingested_at       timestamptz not null default now()
);

create unique index if not exists l_email_source_unique
    on l_email (coalesce(source_account, ''), source_message_id);

create index if not exists l_email_from_idx on l_email (from_address);
create index if not exists l_email_sent_idx on l_email (sent_at desc);

comment on table l_email is
    'LANDING for Google email. One row per message, replay-safe on (source_account, source_message_id). No judgment, no merging; promotion into canonical happens later by identity.';

commit;
