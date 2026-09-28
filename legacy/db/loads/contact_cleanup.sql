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

begin;

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

commit;
