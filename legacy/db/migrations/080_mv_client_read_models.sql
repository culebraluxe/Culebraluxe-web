-- CulebraLuxe Portal
-- CLIENT READ MODEL HARDENING — materialized application read models.
--
-- POSTGRES BUILDS THE READ MODEL ONCE. THE APPLICATION PAGES THE PRE-SHAPED
-- RESULT. The Client UI no longer rebuilds the same multi-table relational
-- projection inside TypeScript on every request.
--
--   SOURCE -> L/ODS -> normalize/reconcile/promote -> canonical tables
--   -> MATERIALIZED APPLICATION READ MODELS -> paged UI
--
-- These are APPLICATION READ MODELS over canonical data. They are NOT
-- replacements for l_person / integration_relationship_evidence.

create extension if not exists pg_trgm;

-- ---------------------------------------------------------------------------
-- mv_client_directory — one row per canonical Person (left People rail).
-- ---------------------------------------------------------------------------
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
        select occurred_at
        from interaction i
        where i.person_id = p.id
        order by i.occurred_at desc
        limit 1
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

-- Indexes supporting the actual access patterns.
create unique index mv_client_directory_pk
    on mv_client_directory (person_id);

-- Left-rail default order: named people first, unknown last, name ASC, id tie-break.
create index mv_client_directory_sort
    on mv_client_directory (name_sort_priority desc, display_name asc, person_id asc);

create index mv_client_directory_status on mv_client_directory (status);
create index mv_client_directory_role   on mv_client_directory (role);
create index mv_client_directory_created on mv_client_directory (created_at desc);
create index mv_client_directory_last_contact on mv_client_directory (last_contact_at desc);

-- Search over the pre-shaped search_text (display name + location + identities).
create index mv_client_directory_search
    on mv_client_directory using gist (search_text gist_trgm_ops);

-- ---------------------------------------------------------------------------
-- mv_client_contact_history — one row per canonical interaction (navy grid).
-- ---------------------------------------------------------------------------
create materialized view mv_client_contact_history as
select
    i.id                         as interaction_id,
    i.person_id,
    i.channel,
    i.direction,
    i.occurred_at,
    to_char(
        i.occurred_at at time zone 'America/Puerto_Rico',
        'Mon FMDD, YYYY HH12:MI AM'
    )                            as occurred_at_label,
    i.title,
    i.summary
from interaction i;

create unique index mv_client_contact_history_pk
    on mv_client_contact_history (interaction_id);

-- Newest-first per person is a simple bounded index-backed read.
create index mv_client_contact_history_person
    on mv_client_contact_history (person_id, occurred_at desc, interaction_id);
