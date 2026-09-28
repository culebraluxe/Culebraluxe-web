-- PRODUCTION, 2026-09-28 — golden people and listing projects, in ONE transaction: all of it happens, or none.
-- Paste the whole file into the SQL console and run it. Safe to run again.
--   1. merge_person() (migration 228)   2. contact cleanup   3. duplicate people merged   4. listing projects
-- The result sets along the way are the reports; the LAST one is the summary.

begin;

-- ==============================================================================================================
-- 1. merge_person() — the merge function (migration 228)   (from legacy/db/migrations/228_person_merge.sql)
-- ==============================================================================================================
-- MERGE TWO RECORDS OF ONE PERSON. Apple split people into several records — one with the phone, one with the email,
-- one empty — and the same person entered twice. merge_person(golden, duplicate) folds the duplicate into the golden
-- record, which is what the Records screen's "Merge into this person" and the duplicate cleanup both call:
--
--   1. every field the golden record lacks (null, or empty text) is filled from the duplicate — the golden record's
--      own values always win;
--   2. everything that points at the duplicate is moved to the golden record: every foreign key to person, found from
--      the catalog (so a table added later is included), and the three references kept as text (project,
--      wbs_item entity, workflow trace). A row the golden record already has an equivalent of (a unique or check
--      rule refuses the move — both were linked to the same property, say) is dropped instead;
--   3. a person linked to themself by the merge is unlinked, and the duplicate is deleted.
--
-- It runs inside the caller's transaction: all of it happens, or none. It answers {"moved": n, "dropped": n}.

create or replace function merge_person(p_golden uuid, p_duplicate uuid) returns jsonb
language plpgsql as $$
declare
    link record;
    row record;
    assignments text;
    moved integer := 0;
    dropped integer := 0;
begin
    if p_golden = p_duplicate then
        raise exception 'merge_person: a person cannot be merged into themself';
    end if;
    perform 1 from person where id in (p_golden, p_duplicate) for update;
    if (select count(*) from person where id in (p_golden, p_duplicate)) <> 2 then
        raise exception 'merge_person: both people must exist';
    end if;

    -- 1. Fill what the golden record lacks.
    select string_agg(
               case when data_type in ('text', 'character varying')
                    then format('%1$I = coalesce(nullif(g.%1$I, %2$L), d.%1$I)', column_name, '')
                    else format('%1$I = coalesce(g.%1$I, d.%1$I)', column_name)
               end, ', ' order by ordinal_position)
      into assignments
      from information_schema.columns
     where table_schema = 'public' and table_name = 'person' and is_generated = 'NEVER'
       and column_name not in ('id', 'created_at', 'updated_at');
    execute format('update person g set %s, updated_at = now() from person d where g.id = %L and d.id = %L',
                   assignments, p_golden, p_duplicate);

    -- 2. Move everything that points at the duplicate.
    for link in
        select c.conrelid::regclass as tbl, a.attname as col
          from pg_constraint c
          join pg_attribute a on a.attrelid = c.conrelid and a.attnum = any (c.conkey)
         where c.contype = 'f' and c.confrelid = 'person'::regclass
    loop
        for row in execute format('select ctid from %s where %I = %L', link.tbl, link.col, p_duplicate) loop
            begin
                execute format('update %s set %I = %L where ctid = %L', link.tbl, link.col, p_golden, row.ctid);
                moved := moved + 1;
            exception when unique_violation or check_violation then
                execute format('delete from %s where ctid = %L', link.tbl, row.ctid);
                dropped := dropped + 1;
            end;
        end loop;
    end loop;
    update project set person_id = p_golden::text where person_id = p_duplicate::text;
    update wbs_item set entity_id = p_golden::text where entity_type = 'person' and entity_id = p_duplicate::text;
    update workflow_execution_trace_event set person_id = p_golden::text where person_id = p_duplicate::text;

    -- 3. No one is their own relation; the duplicate goes.
    delete from person_person where person_id = related_person_id;
    delete from person where id = p_duplicate;

    return jsonb_build_object('moved', moved, 'dropped', dropped);
end
$$;

-- ==============================================================================================================
-- 2. Contact cleanup — phones +1XXXXXXXXXX, names cleaned   (from legacy/db/loads/contact_cleanup.sql)
-- ==============================================================================================================
-- CONTACT CLEANUP — people's names and phones made golden.
--
-- Contacts arrived from Apple (and by hand) in whatever shape they were typed: the same phone as "+17875551234",
-- "(787) 555-1234" and "7875551234", often more than one of them on the same person; names with double spaces,
-- stray quote marks, or in ALL CAPS. This makes what is stored consistent. It is safe to run again: a second run
-- changes nothing.
--
-- What it does, automatically:
--   1. Phones: a US / Puerto Rico number (10 digits, or 11 starting with 1) is stored as +1XXXXXXXXXX.
--   2. The same phone stored more than once on one person, in different formats, is kept once — the primary copy,
--      else the oldest — and the others are deleted.
--   3. Names: trimmed, inner spaces collapsed, wrapping quote marks removed; a name in ALL CAPS gets normal
--      capitals ("LAMKEN WAYNE" -> "Lamken Wayne"). Mixed-case names are left as typed.
--   4. The client read models that copy names and phones are refreshed.
--
-- What it only REPORTS, for a person to decide (nothing is merged):
--   a. Numbers that are not US / Puerto Rico, or have an odd length — left as stored.
--   b. The same phone on two different people.
--   c. The same name (words in any order, ignoring case and punctuation) on two or more people.

-- 1 + 2. Phones -----------------------------------------------------------------------------------------------------

create temporary table contact_phone on commit drop as
select id,
       person_id,
       identity_value,
       is_primary,
       created_at,
       case
           when regexp_replace(identity_value, '[^0-9]', '', 'g') ~ '^[0-9]{10}$'
               then '+1' || regexp_replace(identity_value, '[^0-9]', '', 'g')
           when regexp_replace(identity_value, '[^0-9]', '', 'g') ~ '^1[0-9]{10}$'
               then '+' || regexp_replace(identity_value, '[^0-9]', '', 'g')
       end as golden
  from person_identity
 where identity_type = 'phone';

-- The copy of each number that stays on each person: the primary one, else the oldest.
create temporary table contact_phone_keep on commit drop as
select distinct on (person_id, golden) id, person_id, golden
  from contact_phone
 where golden is not null
 order by person_id, golden, is_primary desc, created_at asc, id;

create temporary table contact_phone_report (step text, n integer) on commit drop;

with removed as (
    delete from person_identity pi
     using contact_phone cp
     where pi.id = cp.id
       and cp.golden is not null
       and not exists (select 1 from contact_phone_keep k where k.id = cp.id)
    returning pi.id
)
insert into contact_phone_report select 'duplicate phone copies removed (same person)', count(*) from removed;

-- A kept copy stays primary if any copy of that number on that person was.
update person_identity pi
   set is_primary = true
  from contact_phone_keep k
 where pi.id = k.id
   and not pi.is_primary
   and exists (select 1 from contact_phone cp where cp.person_id = k.person_id and cp.golden = k.golden and cp.is_primary);

-- Reformat, unless another person already holds the golden form (that is report b, not a change to make here).
with changed as (
    update person_identity pi
       set identity_value = k.golden,
           updated_at = now()
      from contact_phone_keep k
     where pi.id = k.id
       and pi.identity_value <> k.golden
       and not exists (
           select 1 from person_identity other
            where other.identity_type = 'phone' and other.identity_value = k.golden and other.id <> k.id
       )
    returning pi.id
)
insert into contact_phone_report select 'phones reformatted to +1XXXXXXXXXX', count(*) from changed;

-- 3. Names ----------------------------------------------------------------------------------------------------------

with cleaned as (
    select id,
           trim(regexp_replace(
               regexp_replace(trim(display_name), '^[''"‘’“”`]+|[''"‘’“”`]+$', '', 'g'),
               '\s+', ' ', 'g')) as tidy
      from person
),
golden as (
    select id,
           case
               when tidy ~ '[A-Z]' and tidy = upper(tidy) and length(tidy) > 3 then initcap(lower(tidy))
               else tidy
           end as name
      from cleaned
),
changed as (
    update person p
       set display_name = g.name,
           updated_at = now()
      from golden g
     where p.id = g.id
       and g.name <> ''
       and p.display_name <> g.name
    returning p.id
)
insert into contact_phone_report select 'names cleaned', count(*) from changed;

-- 4. Read models ----------------------------------------------------------------------------------------------------

refresh materialized view mv_client_directory;
refresh materialized view mv_client_contact_history;
refresh materialized view mv_client_relationship_channels;

-- Reports -----------------------------------------------------------------------------------------------------------

select step, n from contact_phone_report order by step;

-- a. Numbers left as stored: not US / Puerto Rico, or an odd length.
select p.display_name as person, pi.identity_value as phone_left_as_stored
  from person_identity pi
  join person p on p.id = pi.person_id
 where pi.identity_type = 'phone' and pi.identity_value !~ '^\+1[0-9]{10}$'
 order by p.display_name;

-- b. One phone, two or more people.
select right(regexp_replace(pi.identity_value, '[^0-9]', '', 'g'), 10) as phone,
       string_agg(distinct p.display_name, ' | ') as people
  from person_identity pi
  join person p on p.id = pi.person_id
 where pi.identity_type = 'phone'
 group by 1
having count(distinct pi.person_id) > 1
 order by 1;

-- c. One name (any word order), two or more people — namesakes or duplicates; each group with its phones and emails.
with keyed as (
    select p.id, p.display_name,
           (select string_agg(word, ' ' order by word)
              from regexp_split_to_table(lower(regexp_replace(p.display_name, '[^[:alnum:] ]', '', 'g')), '\s+') word
             where word <> '') as name_key
      from person p
)
select k.name_key,
       count(*) as people,
       string_agg(k.display_name || coalesce(' (' || (
           select string_agg(pi.identity_value, ', ') from person_identity pi
            where pi.person_id = k.id and pi.identity_type in ('phone', 'email')) || ')', ' (no phone or email)'),
           ' | ') as who
  from keyed k
 where k.name_key is not null
 group by k.name_key
having count(*) > 1
 order by count(*) desc, k.name_key;

-- ==============================================================================================================
-- 3. Duplicate people merged where it is safe   (from legacy/db/loads/person_merge_duplicates.sql)
-- ==============================================================================================================
-- DUPLICATE PEOPLE, MERGED WHERE IT IS SAFE. Needs migration 228 (merge_person).
--
-- People are grouped by name — the same words in any order, ignoring case and punctuation ("LAMKEN WAYNE" = "Wayne
-- Lamken"). In each group the GOLDEN record is the one already linked to real work (a property, deal, contract,
-- form, document or project), then the one with the most phones and emails, then the most recently edited.
--
-- A record merges into the golden one only when it is plainly the same person:
--   * it shares a phone (same last ten digits) or an email with the golden record, or
--   * one of the two has no phone and no email at all — an empty copy — AND every record in the group that does have
--     a phone or email shares one with the golden record (so an empty "Dawn" is never folded into one of three
--     different Dawns).
-- Everything else is left alone and listed for a person to decide. Safe to run again.

create temporary table merge_person_member on commit drop as
with keyed as (
    select p.id,
           p.display_name,
           p.updated_at,
           (select string_agg(word, ' ' order by word)
              from regexp_split_to_table(lower(regexp_replace(p.display_name, '[^[:alnum:] ]', '', 'g')), '\s+') word
             where word <> '') as name_key
      from person p
)
select k.*,
       (  (select count(*) from property x where x.seller_person_id = k.id)
        + (select count(*) from person_property x where x.person_id = k.id)
        + (select count(*) from deal x where x.client_person_id = k.id)
        + (select count(*) from deal_participant x where x.person_id = k.id)
        + (select count(*) from contract_person x where x.person_id = k.id)
        + (select count(*) from document_form_instance x where x.person_id = k.id)
        + (select count(*) from document_form_participant x where x.person_id = k.id)
        + (select count(*) from transaction_document x where x.party_person_id = k.id)
        + (select count(*) from offer x where x.person_id = k.id)
        + (select count(*) from project x where x.person_id = k.id::text)) as linked_work,
       array(select case when pi.identity_type = 'phone'
                         then 'phone:' || right(regexp_replace(pi.identity_value, '[^0-9]', '', 'g'), 10)
                         else 'email:' || lower(trim(pi.identity_value)) end
               from person_identity pi
              where pi.person_id = k.id and pi.identity_type in ('phone', 'email')) as contacts
  from keyed k
 where k.name_key is not null
   and k.name_key in (select name_key from keyed where name_key is not null group by name_key having count(*) > 1);

create temporary table merge_person_golden on commit drop as
select distinct on (name_key) name_key, id as golden_id, display_name as golden_name, contacts as golden_contacts
  from merge_person_member
 order by name_key, linked_work desc, cardinality(contacts) desc, updated_at desc, id;

-- A group is ONE PERSON when every record with a phone or email shares one with the golden record — or, when the
-- golden record has none, when at most one record has any.
create temporary table merge_person_group on commit drop as
select g.*,
       case when cardinality(g.golden_contacts) > 0
            then not exists (
                select 1 from merge_person_member o
                 where o.name_key = g.name_key and o.id <> g.golden_id
                   and cardinality(o.contacts) > 0 and not (o.contacts && g.golden_contacts))
            else (select count(*) from merge_person_member o
                   where o.name_key = g.name_key and cardinality(o.contacts) > 0) <= 1
       end as one_person
  from merge_person_golden g;

create temporary table merge_person_plan on commit drop as
select m.id as duplicate_id, m.display_name as duplicate_name, g.golden_id, g.golden_name, m.name_key,
       case when m.contacts && g.golden_contacts then 'shares a phone or email' else 'an empty copy' end as reason
  from merge_person_member m
  join merge_person_group g using (name_key)
 where m.id <> g.golden_id
   and (m.contacts && g.golden_contacts
        or (g.one_person and (cardinality(m.contacts) = 0 or cardinality(g.golden_contacts) = 0)));

create temporary table merge_person_done (golden_name text, duplicate_name text, reason text, result jsonb) on commit drop;

do $$
declare
    step record;
begin
    for step in select * from merge_person_plan order by golden_name, duplicate_name loop
        insert into merge_person_done
        values (step.golden_name, step.duplicate_name, step.reason, merge_person(step.golden_id, step.duplicate_id));
    end loop;
end
$$;

refresh materialized view mv_client_directory;
refresh materialized view mv_client_contact_history;
refresh materialized view mv_client_relationship_channels;

-- What was merged.
select golden_name as kept, duplicate_name as merged_into_it, reason, result from merge_person_done order by 1, 2;

-- What is left for a person to decide: names still on more than one record, with each record's phones and emails.
with keyed as (
    select p.id, p.display_name,
           (select string_agg(word, ' ' order by word)
              from regexp_split_to_table(lower(regexp_replace(p.display_name, '[^[:alnum:] ]', '', 'g')), '\s+') word
             where word <> '') as name_key
      from person p
)
select k.name_key as left_for_review, count(*) as records,
       string_agg(k.display_name || coalesce(' (' || (
           select string_agg(pi.identity_value, ', ') from person_identity pi
            where pi.person_id = k.id and pi.identity_type in ('phone', 'email')) || ')', ' (no phone or email)'),
           ' | ') as who
  from keyed k
 where k.name_key is not null
 group by k.name_key
having count(*) > 1
 order by count(*) desc, k.name_key;

-- ==============================================================================================================
-- 4. Listing projects — one per live listing, six steps   (from legacy/db/loads/listing_projects.sql)
-- ==============================================================================================================
-- LISTING PROJECTS — one project per live listing, each with the listing's six steps.
--
-- Until a signed listing contract creates its project automatically, this is how a live listing gets one. It is safe
-- to run again (a project or step that exists is left as it is), and it runs the same on DEV and PROD: each project is
-- linked to its property — and through the property to its seller — where that listing exists in the database.
--
-- A listing is matched by its name with spaces and case ignored ("Zoni Bluff" = "ZoniBluff"), among properties that
-- are not archived. When no property, or more than one, matches, the project is still created, unlinked; the report
-- at the end says which.
--
-- The six steps start open; mark them done on the Projects screen as they are confirmed.

create temporary table listing_project_load (
    project_id text primary key,
    name text not null,
    listing_key text not null
) on commit drop;

insert into listing_project_load (project_id, name, listing_key) values
    ('listing-alturas-de-zoni-solar-6', 'Alturas de Zoni Solar 6 Listing', 'alturasdezonisolar6'),
    ('listing-crown-paradise',          'Crown Paradise Listing',          'crownparadise'),
    ('listing-zoni-bluff',              'Zoni Bluff Listing',              'zonibluff'),
    ('listing-horizon-bay',             'Horizon Bay Listing',             'horizonbay');

create temporary table listing_project_step (
    suffix text primary key,
    title text not null,
    category text not null,
    sort_order integer not null
) on commit drop;

insert into listing_project_step (suffix, title, category, sort_order) values
    ('contract-created', 'Listing Contract Created',          'contracts', 1),
    ('contract-signed',  'Listing Contract Signed',           'contracts', 2),
    ('media-shot',       'Property Media Shot',               'media',     3),
    ('website-added',    'Property Listing Added to Website', 'marketing', 4),
    ('media-uploaded',   'Property Media Uploaded to Listing','media',     5),
    ('qa-live',          'QA: Property Listing Live in PROD', 'marketing', 6);

-- The one property each listing names, when there is exactly one.
create temporary table listing_project_match on commit drop as
select l.project_id,
       (array_agg(p.id::text))[1] as property_id,
       (array_agg(p.seller_person_id::text))[1] as person_id,
       count(p.id) as matches
  from listing_project_load l
  left join property p
         on lower(regexp_replace(p.name, '\s', '', 'g')) = l.listing_key
        and p.archived_at is null
 group by l.project_id;

insert into project (id, name, status, description, areas, project_type, property_id, person_id)
select l.project_id,
       l.name,
       'open',
       'Listing project: from the listing contract to the listing live on the website.',
       array['contracts', 'media', 'marketing'],
       'listing',
       case when m.matches = 1 then m.property_id end,
       case when m.matches = 1 then m.person_id end
  from listing_project_load l
  join listing_project_match m using (project_id)
on conflict (id) do nothing;

insert into wbs_item (id, project_id, title, category, status, sort_order)
select l.project_id || '-' || s.suffix, l.project_id, s.title, s.category, 'open', s.sort_order
  from listing_project_load l
 cross join listing_project_step s
on conflict (id) do nothing;

-- What was loaded, and what each project is linked to.
select l.name,
       m.matches as properties_matched,
       pr.name as property,
       pe.display_name as person,
       (select count(*) from wbs_item w where w.project_id = l.project_id) as steps
  from listing_project_load l
  join listing_project_match m using (project_id)
  left join project pj on pj.id = l.project_id
  left join property pr on pr.id::text = pj.property_id
  left join person pe on pe.id::text = pj.person_id
 order by l.name;

-- ==============================================================================================================
-- 5. Listing projects take their property's seller (for a project created before the seller was linked)
-- ==============================================================================================================
update project pj
   set person_id = coalesce(p.seller_person_id::text,
                            (select pp.person_id::text from person_property pp where pp.property_id = p.id limit 1)),
       updated_at = now()
  from property p
 where pj.id in ('listing-alturas-de-zoni-solar-6', 'listing-crown-paradise', 'listing-zoni-bluff', 'listing-horizon-bay')
   and pj.property_id = p.id::text
   and pj.person_id is null;

-- ==============================================================================================================
-- Summary
-- ==============================================================================================================
select 'people' as what, count(*)::text as value from person
union all select 'phones not +1XXXXXXXXXX (foreign or odd, left as stored)',
       count(*)::text from person_identity where identity_type = 'phone' and identity_value !~ '^\+1[0-9]{10}$'
union all select 'duplicates merged by this run', count(*)::text from merge_person_done
union all select 'names still on more than one person (for review)', count(*)::text from (
    select (select string_agg(word, ' ' order by word)
              from regexp_split_to_table(lower(regexp_replace(display_name, '[^[:alnum:] ]', '', 'g')), '\s+') word
             where word <> '') k
      from person group by 1 having count(*) > 1) x
union all select 'listing project: ' || pj.name,
       coalesce(pr.name, '(no property)') || ' · ' || coalesce(pe.display_name, '(no person)')
  from project pj
  left join property pr on pr.id::text = pj.property_id
  left join person pe on pe.id::text = pj.person_id
 where pj.id like 'listing-%';

commit;
