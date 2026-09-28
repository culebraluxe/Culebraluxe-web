-- CulebraLuxe Portal
-- Google Calendar provider token store (CRM-08)
-- Migration: 041_google_calendar_token_store.sql
--
-- PROVIDER-SIDE storage, BEHIND the CalendarProvider seam. Holds ONLY the
-- short-lived OAuth ACCESS token that the Google Calendar adapter refreshes
-- from env credentials (GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET /
-- GOOGLE_REFRESH_TOKEN — the long-lived secrets stay in env and are never
-- written to the database).
--
-- This table is NOT canonical CRM data: it is isolated from interaction /
-- person / task and is never referenced by CRM code. Provider tokens are
-- explicitly REJECTED from canonical tables; this provider-scoped store is
-- their only durable home (mirroring how BoldSign provider ids live only in
-- bold_sign_request, migration 037).
--
-- Additive: no existing table or row is changed. No backfill.

begin;

create table if not exists google_calendar_token_store (
    -- the calendar adapter's account namespace ('calendar:google:<namespace>')
    account_namespace text primary key,

    access_token text not null,
    access_token_expires_at timestamptz not null,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

commit;
