-- CulebraLuxe
-- ODS: the Apple Mail landing table.
-- Migration: 159_l_apple_mail.sql
--
-- WHY A SECOND EMAIL TABLE: l_email is the Gmail landing table. Apple Mail is a
-- DIFFERENT mailbox (the captain's work account, in active use), so per the
-- golden rule it gets its own landing table rather than being mixed into
-- l_email's source accounts. Each source stays a separate pull.
--
-- Same contract as every other landing table, deliberately, so ONE promote path
-- serves all of them:
--   * source_account + source_message_id is the replay key
--   * raw carries the untouched source record (the parsed .emlx / MIME, or
--     whatever the extractor produces) — a landing row is evidence, not
--     interpretation
--   * ingested_at is ours, never the message's own date
--
-- `mailbox` is the one field l_email does not need: Apple Mail has real folders
-- (INBOX, Sent, Archive) and the folder is part of what the record IS, not a
-- label applied after the fact.
--
-- NOTE: there is no loader yet. The captain extracts from Apple separately; this
-- table is created ahead of that so the landing target exists and the contract is
-- fixed. It stays empty until an extractor lands rows. That is expected.

begin;

create table if not exists l_apple_mail (
    id                uuid primary key default gen_random_uuid(),
    source_account    text,
    -- RFC Message-ID when present, otherwise Apple's own row/emlx identity.
    source_message_id text not null,
    -- the conversation this belongs to (References / In-Reply-To chain)
    thread_id         text,
    -- which folder it sat in: INBOX, Sent, Archive, ...
    mailbox           text,
    from_address      text,
    to_address        text,
    cc_address          text,
    subject           text,
    sent_at           timestamptz,
    body_preview      text,
    has_attachments   boolean,
    is_read           boolean,
    raw               jsonb,
    ingested_at       timestamptz not null default now()
);

create unique index if not exists l_apple_mail_source_unique
    on l_apple_mail (coalesce(source_account, ''), source_message_id);

create index if not exists l_apple_mail_from_idx on l_apple_mail (from_address);
create index if not exists l_apple_mail_sent_idx on l_apple_mail (sent_at desc);
create index if not exists l_apple_mail_mailbox_idx on l_apple_mail (mailbox);

comment on table l_apple_mail is
    'LANDING for Apple Mail (the work mailbox). One row per message, replay-safe on (source_account, source_message_id). No judgment, no merging, no identity resolution.';

commit;
