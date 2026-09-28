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

begin;

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

commit;
