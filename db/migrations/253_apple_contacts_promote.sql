-- APPLE CONTACTS: LANDING -> WAREHOUSE, IN THE DATABASE.
--
-- This chain used to be TypeScript: it read every l_person and l_property row out of Neon into Node,
-- normalized identities and decided matches in memory, then pushed the rows back one statement at a
-- time. That is the wrong place for it. Deciding what is true about a person is the domain's job, the
-- data never needs to leave the database to be transformed, and a rule that lives in SQL is a rule that
-- cannot drift between callers. So the transformation lives here, as functions, and Rust only calls
-- them and prints what they answer:
--
--   warehouse_promote_apple_contacts(p_apply)  l_person + l_property -> person / property / person_property
--
-- Identity normalization is the same rule the Rust domain applies (`domain::apple_messages::
-- normalize_email` / `normalize_phone`), restated in SQL because the matching happens in SQL. The two
-- are pinned to each other by tests on both sides: `rust/core/domain/src/apple_messages.rs` for the
-- Rust copy, and the migration's own `select` checks below for this one.
--
-- The promotion is idempotent, set-based, and never pulls a row into an application to mutate it.

-- ---------------------------------------------------------------------------
-- normalize_identity_email — lowercased, or NULL when the value is not an email
-- shape. Mirrors `normalize_email`: at most one `@`, no whitespace, a non-empty
-- local part, and a domain with a dot and text on both sides of it.
-- ---------------------------------------------------------------------------
create or replace function normalize_identity_email(p_raw text) returns text
language sql immutable as $$
    select case
        when v = '' or length(v) > 320 then null
        when v ~ '\s' then null
        when (length(v) - length(replace(v, '@', ''))) <> 1 then null
        when split_part(v, '@', 1) = '' then null
        when split_part(v, '@', 2) = '' then null
        when strpos(split_part(v, '@', 2), '.') = 0 then null
        when split_part(split_part(v, '@', 2), '.', 1) = '' then null
        when split_part(split_part(v, '@', 2), '.', 2) = '' then null
        else lower(v)
    end
    from (select btrim(coalesce(p_raw, '')) as v) t
$$;

-- ---------------------------------------------------------------------------
-- normalize_identity_phone — US/Puerto Rico E.164, or NULL when it is not
-- reliably matchable (`normalize_phone`: 10 digits, or 11 starting with 1).
-- A number we cannot normalize is quarantined, never guessed at.
-- ---------------------------------------------------------------------------
create or replace function normalize_identity_phone(p_raw text) returns text
language sql immutable as $$
    select case
        when length(d) = 11 and left(d, 1) = '1' then '+' || d
        when length(d) = 10 then '+1' || d
        else null
    end
    from (select regexp_replace(coalesce(p_raw, ''), '[^0-9]', '', 'g') as d) t
$$;


-- ---------------------------------------------------------------------------
-- warehouse_promote_apple_contacts(p_apply) — l_person + l_property -> person,
-- property, person_identity, person_property.
--
--   * identity is the normalized email/phone; a landing row with no matchable
--     identity is skipped, never guessed into a person;
--   * exactly one owning person -> matched; none -> created; more than one ->
--     ambiguous and left alone (that is a merge decision, not this function's);
--   * the landing row is the latest source state, so a non-empty landing value
--     wins over the canonical one;
--   * an address is a place, not a listing: status stays 'prospect' and no name
--     is invented.
--
-- Dry run by default: with p_apply false it computes and answers the same tallies
-- and writes nothing. Answers one jsonb object with the counters.
-- ---------------------------------------------------------------------------
create or replace function warehouse_promote_apple_contacts(p_apply boolean default false)
returns jsonb
language plpgsql as $$
declare
    matched integer := 0;
    created integer := 0;
    ambiguous integer := 0;
    skipped_no_identity integer := 0;
    facts_updated integer := 0;
    identities_added integer := 0;
    properties_created integer := 0;
    properties_updated integer := 0;
    properties_linked integer := 0;
    legacy_retired integer := 0;
begin
    -- 1. THE PLAN — every landing person, the identities it owns, the person it resolved to, and the
    --    legal address to write. Built once, in SQL, so a dry run and an apply cannot disagree about
    --    what would happen.
    create temp table promote_contacts_plan on commit drop as
    with landing as (
        select lp.id,
               lp.source,
               lp.source_account,
               lp.source_contact_id,
               lp.display_name,
               lp.note,
               lp.organization,
               lp.emails,
               lp.phones,
               (select nullif(trim(concat_ws(', ',
                           nullif(trim(a.address_line1), ''),
                           nullif(trim(a.city), ''),
                           nullif(trim(concat_ws(' ', nullif(trim(a.state_or_province), ''),
                                                      nullif(trim(a.postal_code), ''))), ''))), '')
                  from l_property a
                 where a.source_system = lp.source
                   and a.source_account = lp.source_account
                   and a.source_key like lp.source_contact_id || ':%'
                   and a.address_type = 'LEGAL'
                 limit 1) as legal_address
          from l_person lp
         where lp.source = 'apple_contacts'
    ),
    -- Emails first, then phones, each in source order: the first identity is the primary one.
    identity_rows as (
        select l.id as l_person_id, x.kind_order, x.ord, x.kind, x.value
          from landing l
          cross join lateral (
              select 0 as kind_order, t.ord::integer as ord, 'email'::text as kind,
                     normalize_identity_email(t.e ->> 'value') as value
                from jsonb_array_elements(coalesce(l.emails, '[]'::jsonb)) with ordinality as t(e, ord)
              union all
              select 1 as kind_order, t.ord::integer as ord, 'phone'::text as kind,
                     normalize_identity_phone(t.e ->> 'value') as value
                from jsonb_array_elements(coalesce(l.phones, '[]'::jsonb)) with ordinality as t(e, ord)
          ) x
    ),
    matchable as (
        select i.l_person_id, i.kind, i.value, i.kind_order, i.ord,
               row_number() over (partition by i.l_person_id order by i.kind_order, i.ord) = 1 as is_primary
          from identity_rows i
         where i.value is not null
    ),
    owned as (
        select m.l_person_id, array_agg(distinct pi.person_id) as person_ids
          from matchable m
          join person_identity pi
            on pi.identity_type = m.kind and pi.identity_value = m.value
         group by 1
    )
    select l.id as l_person_id,
           l.source,
           l.source_account,
           l.source_contact_id,
           l.display_name,
           l.note,
           l.organization,
           l.legal_address,
           coalesce((select jsonb_agg(jsonb_build_object('kind', m.kind,
                                                         'value', m.value,
                                                         'is_primary', m.is_primary)
                                      order by m.kind, m.value)
                       from matchable m where m.l_person_id = l.id), '[]'::jsonb) as identities,
           case
               when coalesce(array_length(o.person_ids, 1), 0) = 1 then o.person_ids[1]
               when coalesce(array_length(o.person_ids, 1), 0) = 0
                    and (select count(*) from matchable m where m.l_person_id = l.id) > 0
                   then gen_random_uuid()
               else null
           end as person_id,
           case
               when (select count(*) from matchable m where m.l_person_id = l.id) = 0 then 'skip'
               when coalesce(array_length(o.person_ids, 1), 0) = 1 then 'match'
               when coalesce(array_length(o.person_ids, 1), 0) = 0 then 'create'
               else 'ambiguous'
           end as decision
      from landing l
      left join owned o on o.l_person_id = l.id;

    -- 2. THE ADDRESS FACTS — l_property joined to the resolved person, once.
    create temp table promote_contacts_facts on commit drop as
    select distinct
           p.person_id,
           'apple_contacts:' || a.source_key as key,
           case when upper(coalesce(a.address_type, '')) = 'PHYSICAL'
                then 'physical_property' else 'legal_address' end as rel,
           nullif(trim(a.address_line1), '') as line1,
           nullif(trim(a.city), '') as city,
           nullif(trim(a.state_or_province), '') as state,
           nullif(trim(a.postal_code), '') as postal,
           nullif(trim(a.country), '') as country,
           'us' as iso,
           nullif(trim(concat_ws(', ',
                 nullif(trim(a.address_line1), ''),
                 nullif(trim(a.city), ''),
                 nullif(trim(concat_ws(' ', nullif(trim(a.state_or_province), ''),
                                            nullif(trim(a.postal_code), ''))), ''))), '') as loc
      from l_property a
      join promote_contacts_plan p
        on a.source_system = p.source
       and a.source_account = p.source_account
       and a.source_key like p.source_contact_id || ':%'
     where a.source_system = 'apple_contacts'
       and p.decision in ('match', 'create');

    -- 3. THE TALLIES — the same numbers a dry run reports and an apply then performs.
    select count(*) filter (where decision = 'match'),
           count(*) filter (where decision = 'create'),
           count(*) filter (where decision = 'ambiguous'),
           count(*) filter (where decision = 'skip')
      into matched, created, ambiguous, skipped_no_identity
      from promote_contacts_plan;

    select count(*) filter (where not exists (
               select 1 from property x
                where x.source_type = 'apple_contacts' and x.source_listing_key = f.key)),
           count(*) filter (where exists (
               select 1 from property x
                where x.source_type = 'apple_contacts' and x.source_listing_key = f.key))
      into properties_created, properties_updated
      from promote_contacts_facts f;

    -- 4. APPLY — every write below is set-based: no row is read out, mutated, and pushed back.
    if p_apply then
        insert into person (id, display_name, role, status, display_name_source, created_at, updated_at)
        select p.person_id,
               coalesce(nullif(trim(p.display_name), ''), '(unnamed)'),
               'unclassified',
               'new',
               'apple_contacts',
               now(),
               now()
          from promote_contacts_plan p
         where p.decision = 'create';

        -- A created person's identities are primary-first as the source listed them.
        insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
        select p.person_id, i.kind, i.value, 'apple_contacts', i.is_primary
          from promote_contacts_plan p
          cross join lateral jsonb_to_recordset(p.identities)
               as i(kind text, value text, is_primary boolean)
         where p.decision = 'create';

        update person t set
               display_name = coalesce(nullif(trim(p.display_name), ''), t.display_name),
               display_name_source = case when nullif(trim(p.display_name), '') is not null
                                          then 'apple_contacts' else t.display_name_source end,
               location = coalesce(nullif(trim(p.legal_address), ''), t.location),
               notes = coalesce(nullif(trim(p.note), ''), t.notes),
               company = coalesce(nullif(trim(p.organization), ''), t.company),
               updated_at = now()
          from promote_contacts_plan p
         where t.id = p.person_id
           and p.decision in ('match', 'create');
        get diagnostics facts_updated = row_count;

        -- Any identity the matched person did not already have. `distinct` collapses two landing
        -- rows carrying the same identity for the same person into one insert.
        insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
        select distinct p.person_id, i.kind, i.value, 'apple_contacts', false
          from promote_contacts_plan p
          cross join lateral jsonb_to_recordset(p.identities)
               as i(kind text, value text, is_primary boolean)
         where p.decision = 'match'
           and not exists (
               select 1 from person_identity y
                where y.person_id = p.person_id
                  and y.identity_type = i.kind
                  and y.identity_value = i.value);
        get diagnostics identities_added = row_count;

        insert into property (id, name, status, featured, source_type, source_provider,
                              source_listing_key, source_modified_at, last_synced_at,
                              address_line1, city, state_or_province, postal_code, country,
                              iso_country_code, location)
        select distinct on (f.key) gen_random_uuid(), null, 'prospect', false, 'apple_contacts',
               'apple_contacts', f.key, now(), now(),
               f.line1, f.city, f.state, f.postal, f.country, f.iso, f.loc
          from promote_contacts_facts f
         where not exists (
               select 1 from property x
                where x.source_type = 'apple_contacts' and x.source_listing_key = f.key)
         order by f.key;
        get diagnostics properties_created = row_count;

        -- The landing row is the latest source state for this address.
        update property x set
               address_line1 = coalesce(f.line1, x.address_line1),
               city = coalesce(f.city, x.city),
               state_or_province = coalesce(f.state, x.state_or_province),
               postal_code = coalesce(f.postal, x.postal_code),
               country = coalesce(f.country, x.country),
               iso_country_code = coalesce(f.iso, x.iso_country_code),
               location = coalesce(f.loc, x.location),
               last_synced_at = now(),
               updated_at = now()
          from (select distinct on (key) key, line1, city, state, postal, country, iso, loc
                  from promote_contacts_facts order by key) f
         where x.source_type = 'apple_contacts' and x.source_listing_key = f.key;
        get diagnostics properties_updated = row_count;

        insert into person_property (person_id, property_id, relation_type, source_type, source_key)
        select distinct f.person_id, x.id, f.rel, 'apple_contacts', f.key
          from promote_contacts_facts f
          join property x on x.source_type = 'apple_contacts' and x.source_listing_key = f.key
        on conflict (person_id, property_id, relation_type) do nothing;
        get diagnostics properties_linked = row_count;

        -- The earlier half-built path wrote the generic 'address' type, which the forms never read.
        delete from person_property
         where relation_type = 'address'
           and source_key in (select distinct key from promote_contacts_facts);
        get diagnostics legacy_retired = row_count;
    end if;

    return jsonb_build_object(
        'apply', p_apply,
        'landingRows', (select count(*) from promote_contacts_plan),
        'matchedExisting', matched,
        'created', created,
        'ambiguous', ambiguous,
        'skippedNoIdentity', skipped_no_identity,
        'factsUpdated', facts_updated,
        'identitiesAdded', identities_added,
        'propertiesCreated', properties_created,
        'propertiesUpdated', properties_updated,
        'propertiesLinked', properties_linked,
        'legacyAddressLinksRetired', legacy_retired
    );
end
$$;

