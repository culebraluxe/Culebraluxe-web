-- CulebraLuxe
-- ODS restructure, step 6: the remaining LANDING tables.
-- Migration: 158_l_call_l_whatsapp_l_calendar.sql
--
-- MODEL (captain, 2026-09-11): the landing layer is the ONLY intake target.
--
--   Apple iMessage  -> l_imessage  (150)
--   Google email    -> l_email     (150)
--   Apple calls /
--   FaceTime        -> l_call      (this migration)
--   WhatsApp Cloud  -> l_whatsapp  (this migration)
--   Calendar        -> l_calendar  (this migration)
--
--   landing -> promote -> warehouse (interaction / person / property)
--
-- WHY: 150 created l_imessage and l_email as the landing stop, but nothing was
-- ever repointed to write into them - every intake still inserts straight into
-- the warehouse. So the landing tables existed with zero writers, and calls,
-- FaceTime, WhatsApp and Calendar had no landing table at all. This completes
-- the set; the intake rewiring follows in code.
--
-- CONTRACT, identical to l_imessage/l_email so one promote path can serve all of
-- them:
--   * source_account      - which account/calendar the record came from
--   * source_message_id   - the SOURCE's stable id; the replay key. For calls
--                           this is Apple's call uniqueId, for calendar the
--                           event UID. The column keeps the uniform name so the
--                           unique index and the promotion logic are identical
--                           across every landing table.
--   * raw                 - the untouched source payload
--   * ingested_at         - when we landed it (never the source's timestamp)
-- One row per source record, replay-safe on (source_account, source_message_id).
-- No judgment, no merging, no identity resolution here.

begin;

create table if not exists l_call (
    id                uuid primary key default gen_random_uuid(),
    source_account    text,
    -- Apple's call uniqueId (the replay key).
    source_message_id text not null,
    -- the other party's phone/email as exported
    handle            text,
    -- 'incoming' | 'outgoing' | null, derived from the source's `originated`
    direction         text,
    -- 'audio' | 'video'; video is FaceTime
    call_type         text,
    answered          boolean,
    duration_seconds  integer,
    started_at        timestamptz,
    raw               jsonb,
    ingested_at       timestamptz not null default now()
);

create unique index if not exists l_call_source_unique
    on l_call (coalesce(source_account, ''), source_message_id);

create index if not exists l_call_handle_idx on l_call (handle);
create index if not exists l_call_started_idx on l_call (started_at desc);

comment on table l_call is
    'LANDING for Apple call history, including FaceTime. One row per call, replay-safe on (source_account, source_message_id). No judgment, no merging.';

create table if not exists l_whatsapp (
    id                uuid primary key default gen_random_uuid(),
    source_account    text,
    -- Meta's message id (wamid) - the replay key.
    source_message_id text not null,
    -- the message this one replies to / threads with
    conversation_id   text,
    from_address      text,
    to_address        text,
    direction         text,
    -- text | image | video | audio | document | sticker | button | interactive
    message_type      text,
    text_content      text,
    -- provider media id, when the message carries media
    media_id          text,
    sent_at           timestamptz,
    raw               jsonb,
    ingested_at       timestamptz not null default now()
);

create unique index if not exists l_whatsapp_source_unique
    on l_whatsapp (coalesce(source_account, ''), source_message_id);

create index if not exists l_whatsapp_from_idx on l_whatsapp (from_address);
create index if not exists l_whatsapp_sent_idx on l_whatsapp (sent_at desc);

comment on table l_whatsapp is
    'LANDING for WhatsApp Cloud messages. One row per message, replay-safe on (source_account, source_message_id). No judgment, no merging.';

create table if not exists l_calendar (
    id                uuid primary key default gen_random_uuid(),
    source_account    text,
    -- the event UID (the replay key).
    source_message_id text not null,
    title             text,
    starts_at         timestamptz,
    ends_at           timestamptz,
    all_day           boolean,
    location          text,
    organizer         text,
    attendees         jsonb,
    raw               jsonb,
    ingested_at       timestamptz not null default now()
);

create unique index if not exists l_calendar_source_unique
    on l_calendar (coalesce(source_account, ''), source_message_id);

create index if not exists l_calendar_starts_idx on l_calendar (starts_at desc);

comment on table l_calendar is
    'LANDING for calendar events (EventKit snapshot). One row per event, replay-safe on (source_account, source_message_id). No judgment, no merging.';

commit;
