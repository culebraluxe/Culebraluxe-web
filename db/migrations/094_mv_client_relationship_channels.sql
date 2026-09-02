-- 094_mv_client_relationship_channels.sql
-- Source-grain client relationship read model.
--
--   relationship evidence
--       -> canonical Person
--       -> ONE current relationship state per Person x communication source
--       -> materialized Client read model
--       -> Client screen (one gold node per source)
--
-- The grain is ONE ROW per (canonical Person, communication source) — never per
-- message, never per interaction, never per conversation burst. Multiple source
-- handles for the same Person+source reduce to one row. Only communication-
-- bearing evidence rows (linked, not bulk, not organization/service) contribute.

-- Dependency order: the directory depends on the channels MV, so it is dropped
-- first (then recreated below). Both drops are idempotent for replay safety.
drop materialized view if exists mv_client_directory;
drop materialized view if exists mv_client_relationship_channels;

create materialized view mv_client_relationship_channels as
with comm as (
    select
        ev.canonical_person_id as person_id,
        ev.source,
        ev.first_observed_at,
        ev.last_observed_at,
        ev.last_inbound_at,
        ev.last_outbound_at,
        coalesce(ev.inbound_count, 0) as inbound_count,
        coalesce(ev.outbound_count, 0) as outbound_count,
        coalesce(ev.is_two_way, false) as is_two_way
    from integration_relationship_evidence ev
    where ev.canonical_person_id is not null
      and ev.review_state = 'exact_linked'
      and (ev.is_automated_or_bulk is not true)
      and (ev.is_organization_or_service is not true)
),
agg as (
    select
        person_id,
        source,
        min(first_observed_at)                                    as first_observed_at,
        max(coalesce(last_observed_at, last_outbound_at, last_inbound_at)) as last_contact_at,
        max(last_inbound_at)                                      as last_inbound_at,
        max(last_outbound_at)                                     as last_outbound_at,
        sum(inbound_count)::bigint                                as inbound_count,
        sum(outbound_count)::bigint                               as outbound_count,
        (sum(inbound_count) + sum(outbound_count))::bigint        as total_count,
        bool_or(is_two_way)                                       as two_way
    from comm
    group by person_id, source
    -- A source with no communication (counts or dates) is not a relationship
    -- channel; identity-only sources (e.g. apple_contacts) never appear here.
    having (sum(inbound_count) + sum(outbound_count)) > 0
        or max(coalesce(last_observed_at, last_outbound_at, last_inbound_at)) is not null
),
-- ONE bounded last context per Person+source: the latest canonical interaction
-- (by provenance source_system) that carries human-readable content. NULL when
-- a source has no such context. This is relationship memory, not a message
-- archive — never thousands of excerpts.
context_candidates as (
    select
        i.person_id,
        coalesce(i.source_system, '') as source,
        i.direction,
        i.occurred_at,
        nullif(btrim(regexp_replace(
            replace(replace(replace(coalesce(i.title, ''), chr(65532), ''), chr(65533), ''), chr(65279), ''),
            '[[:cntrl:]]+', ' ', 'g'
        )), '') as clean_title,
        nullif(btrim(regexp_replace(
            replace(replace(replace(coalesce(i.summary, ''), chr(65532), ''), chr(65533), ''), chr(65279), ''),
            '[[:cntrl:]]+', ' ', 'g'
        )), '') as clean_summary
    from interaction i
),
contexts as (
    select
        c.*,
        case
            when c.source in ('apple_messages', 'whatsapp')
            then case
                when lower(c.clean_summary) not in ('message', 'attachment') then c.clean_summary
                when lower(c.clean_title) not in ('message', 'attachment') then c.clean_title
                else null
            end
            else coalesce(c.clean_title, c.clean_summary)
        end as context_value,
        case
            when c.source in ('apple_messages', 'whatsapp')
                 and lower(c.clean_summary) not in ('message', 'attachment') then 'summary'
            when c.clean_title is not null then 'title'
            when c.clean_summary is not null then 'summary'
            else null
        end as context_type
    from context_candidates c
),
ctx as (
    select distinct on (person_id, source)
        person_id,
        source,
        context_value as last_context,
        context_type as last_context_type,
        direction as last_context_direction,
        occurred_at
    from contexts
    where context_value is not null
    order by person_id, source, occurred_at desc
)
select
    a.person_id,
    a.source,
    (case
        when a.source = 'apple_messages' then 'imessage'
        when a.source = 'gmail_contacts' then 'email'
        when a.source = 'gmail'          then 'email'
        else a.source
    end) as channel,
    a.first_observed_at,
    a.last_contact_at,
    a.last_inbound_at,
    a.last_outbound_at,
    a.inbound_count,
    a.outbound_count,
    a.total_count,
    (case
        when a.last_inbound_at is not null
             and (a.last_outbound_at is null or a.last_inbound_at > a.last_outbound_at)
        then 'inbound'
        when a.last_outbound_at is not null then 'outbound'
        else null
    end) as last_direction,
    a.two_way,
    c.last_context       as last_context,
    c.occurred_at        as last_context_at,
    c.last_context_type  as last_context_type,
    c.last_context_direction as last_context_direction
from agg a
left join ctx c on c.person_id = a.person_id and c.source = a.source;

create unique index mv_client_relationship_channels_pk
    on mv_client_relationship_channels (person_id, source);

create index mv_client_relationship_channels_contact
    on mv_client_relationship_channels (person_id, last_contact_at desc nulls last);

-- ---------------------------------------------------------------------------
-- mv_client_directory — one row per canonical Person (left People rail).
-- Relationship/source state is now AUTHORITATIVE for freshness: a Person must
-- not appear stale merely because a communication source is represented only by
-- aggregate relationship evidence. Detailed interaction is a fallback when a
-- Person has no source-channel rows yet.
-- ---------------------------------------------------------------------------
drop materialized view if exists mv_client_directory;

create materialized view mv_client_directory as
with base as (
    select
        p.id                          as person_id,
        p.display_name,
        p.display_name_source,
        p.role,
        p.status,
        p.location,
        p.created_at,
        p.assigned_user_id,
        email.identity_value          as primary_email,
        phone.identity_value          as primary_phone,
        u.display_name                as assigned_agent,
        latest.occurred_at            as last_contact_at,
        to_char(
            latest.occurred_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY'
        )                             as last_contact_label,
        coalesce(evidence.sources, '{}'::text[]) as sources,
        coalesce(identity.search_text, '')       as search_text
    from person p
    left join app_user u on u.id = p.assigned_user_id
    left join lateral (
        select identity_value
        from person_identity pi
        where pi.person_id = p.id and pi.identity_type = 'email'
        order by pi.is_primary desc, pi.created_at asc
        limit 1
    ) email on true
    left join lateral (
        select identity_value
        from person_identity pi
        where pi.person_id = p.id and pi.identity_type = 'phone'
        order by pi.is_primary desc, pi.created_at asc
        limit 1
    ) phone on true
    left join lateral (
        -- Source-grain relationship state is authoritative; detailed interaction
        -- is only a fallback when no source channel exists yet.
        select coalesce(
            (select max(rc.last_contact_at)
             from mv_client_relationship_channels rc
             where rc.person_id = p.id),
            (select max(i.occurred_at)
             from interaction i
             where i.person_id = p.id)
        ) as occurred_at
    ) latest on true
    left join lateral (
        select array_agg(distinct source order by source) as sources
        from integration_relationship_evidence ev
        where ev.canonical_person_id = p.id
    ) evidence on true
    left join lateral (
        select string_agg(identity_value, ' ') as search_text
        from person_identity pi
        where pi.person_id = p.id
    ) identity on true
    where p.archived_at is null
)
select
    b.*,
    (case
        when b.display_name_source = 'unresolved'
            or b.display_name is null
            or trim(b.display_name) = ''
            or lower(b.display_name) = 'unknown contact'
            or b.display_name ~ '^[+0-9()\s.-]+$'
        then 0
        else 1
    end) as name_sort_priority
from base b;

create unique index mv_client_directory_pk
    on mv_client_directory (person_id);

create index mv_client_directory_sort
    on mv_client_directory (name_sort_priority desc, display_name asc, person_id asc);

create index mv_client_directory_status on mv_client_directory (status);
create index mv_client_directory_role   on mv_client_directory (role);
create index mv_client_directory_created on mv_client_directory (created_at desc);
create index mv_client_directory_last_contact on mv_client_directory (last_contact_at desc);

create index mv_client_directory_search
    on mv_client_directory using gist (search_text gist_trgm_ops);
