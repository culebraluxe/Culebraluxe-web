-- 095_mv_client_directory_name_search.sql
-- Client directory READ-MODEL correction (no canonical Person mutation).
--
-- Defect 1 — search_text was assembled ONLY from person_identity.identity_value,
-- so the canonical display_name (and location) were NOT searchable. The original
-- comment claimed "display name + location + identities", but the SQL did not do
-- that. Correct search_text to include bounded searchable directory text:
--   canonical display_name + location + all identity values.
--
-- Defect 2 — name_sort_priority only demoted display_name_source = 'unresolved'.
-- Canonical Persons whose display_name is an unresolved identity fallback
-- (e.g. "...:ABPerson" structured Apple identifiers) were sorting among genuine
-- resolved human names. Demote 'identity_fallback' (and any structured
-- "...:Suffix" display value) to the unresolved/low sort priority.

-- Dependency-safe: mv_client_relationship_channels and mv_client_contact_history
-- are NOT touched; only mv_client_directory is recreated.

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
        -- Searchable directory text: display name + location + identities.
        trim(coalesce(
            concat_ws(' ', p.display_name, p.location, identity.search_text),
            ''
        ))                            as search_text
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
    -- Unresolved display values sort last: identity_fallback and structured
    -- identifiers (e.g. "...:ABPerson") are honest unresolved, never resolved
    -- human names. Genuine source/human names keep the resolved/high priority.
    (case
        when b.display_name_source in ('unresolved', 'identity_fallback')
            or b.display_name is null
            or trim(b.display_name) = ''
            or lower(b.display_name) = 'unknown contact'
            or b.display_name ~ '^[+0-9()\s.-]+$'
            or b.display_name ~ ':[A-Za-z0-9]+$'
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
