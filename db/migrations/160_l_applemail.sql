-- CulebraLuxe
-- Apple Mail landing table.
-- Migration: 160_l_applemail.sql
--
-- SOURCE -> L TABLE -> PROMOTION/RECONCILIATION -> WAREHOUSE
-- The L table is SOURCE EVIDENCE. Landing happens BEFORE identity
-- reconciliation, Person matching, ambiguity filtering, or interaction creation.
--
--   Gmail API          -> l_email       (Google)
--   authenticated Mail.app -> JXA bridge -> l_applemail   (Apple)
--
-- Two independent source adapters feeding the same broader email domain. Apple
-- Mail is NOT "Gmail with a different account", so it gets its own table and its
-- own provenance. Both may converge later during promotion.
--
-- Metadata only. The acquisition layer deliberately retrieves no body, no
-- snippet, no attachments and no raw MIME, and this table stores none of them.
--
-- SUPERSEDES 159 (`l_apple_mail`): created earlier the same day, before this
-- story fixed the name and the shape. It was never written to and is empty, so
-- it is dropped rather than left as a second, competing Apple table.

begin;

drop table if exists l_apple_mail;

create table if not exists l_applemail (
    id              uuid primary key default gen_random_uuid(),
    -- the configured Mail.app account address, e.g. lisa@culebraluxe.com
    source_account  text,
    -- the replay identity: 'message-id:<Message-ID>' when Apple exposes one,
    -- else 'mail-local:<mailbox-kind>:<local-id>'. Never random.
    source_message_id text not null,
    -- 'inbox' | 'sent'
    mailbox_kind    text,
    -- the Mail.app mailbox folder name
    mailbox_name    text,
    -- Mail.app's own local row id (bigint: it is an integer sequence on disk)
    local_id        bigint,
    -- the RFC Message-ID, when present, kept alongside the replay key
    message_id      text,
    occurred_at     timestamptz,
    sender          text,
    to_recipients   jsonb,
    cc_recipients   jsonb,
    bcc_recipients  jsonb,
    subject         text,
    -- the UNMODIFIED record the JXA exporter produced for this message
    raw             jsonb not null,
    ingested_at     timestamptz not null default now()
);

create unique index if not exists l_applemail_source_unique
    on l_applemail (coalesce(source_account, ''), source_message_id);

create index if not exists l_applemail_sender_idx on l_applemail (sender);
create index if not exists l_applemail_occurred_idx on l_applemail (occurred_at desc);
create index if not exists l_applemail_mailbox_idx on l_applemail (mailbox_kind);

comment on table l_applemail is
    'LANDING for Apple Mail metadata (authenticated Mail.app via the JXA bridge). One row per message, replay-safe on (source_account, source_message_id). Source evidence only: no identity resolution, no Person matching, no CRM decision, and nothing is promoted from here.';

commit;
