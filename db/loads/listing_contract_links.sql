-- LISTING CONTRACTS LINKED BY CATASTRO. A listing contract's legal description is typed with care — its catastro
-- number is the golden key — so a contract with no property (or with a blank stub property: no name, no catastro,
-- made when the contract was) is linked to the one live property carrying that catastro,
-- and a contract with no person to the one person whose name is the seller's (the same words in any order, titles
-- like "Dr." ignored). Nothing already linked is changed; an ambiguous or missing match is left alone. Safe to run
-- again.
--
-- Also recorded: Horizon Bay's listing contract is signed (the owner's word, 2026-09-28) — its project step is done.

begin;

with linked as (
    update document_form_instance f
       set property_id = (
               select p.id from property p
                where p.archived_at is null
                  and regexp_replace(coalesce(p.catastro_number, ''), '[^0-9]', '', 'g')
                    = regexp_replace(f.field_values ->> 'catastroNumber', '[^0-9]', '', 'g')),
           updated_at = now()
     where f.template_id = 'LISTING-01'
       -- unlinked, or linked to a blank stub (no name, no catastro) standing in for the real property
       and (f.property_id is null
            or exists (select 1 from property stub
                        where stub.id = f.property_id
                          and coalesce(trim(stub.name), '') = '' and coalesce(stub.catastro_number, '') = ''))
       and coalesce(regexp_replace(f.field_values ->> 'catastroNumber', '[^0-9]', '', 'g'), '') <> ''
       and (select count(*) from property p
             where p.archived_at is null
               and regexp_replace(coalesce(p.catastro_number, ''), '[^0-9]', '', 'g')
                 = regexp_replace(f.field_values ->> 'catastroNumber', '[^0-9]', '', 'g')) = 1
    returning f.id
)
select 'contracts linked to their property' as step, count(*) as n from linked;

with name_key as (
    select id,
           (select string_agg(word, ' ' order by word)
              from regexp_split_to_table(lower(regexp_replace(display_name, '[^[:alnum:] ]', '', 'g')), '\s+') word
             where word <> '' and word not in ('dr', 'mr', 'mrs', 'ms')) as k
      from person
),
contract as (
    select f.id,
           (select string_agg(word, ' ' order by word)
              from regexp_split_to_table(lower(regexp_replace(f.field_values ->> 'sellerName', '[^[:alnum:] ]', '', 'g')), '\s+') word
             where word <> '' and word not in ('dr', 'mr', 'mrs', 'ms')) as k
      from document_form_instance f
     where f.template_id = 'LISTING-01' and f.person_id is null and coalesce(f.field_values ->> 'sellerName', '') <> ''
),
linked as (
    update document_form_instance f
       set person_id = (select n.id from name_key n where n.k = c.k),
           updated_at = now()
      from contract c
     where f.id = c.id
       and (select count(*) from name_key n where n.k = c.k) = 1
    returning f.id
)
select 'contracts linked to their seller' as step, count(*) as n from linked;

update wbs_item
   set status = 'done', updated_at = now()
 where id = 'listing-horizon-bay-contract-signed' and status <> 'done';

-- Every listing contract, as it now stands.
select f.created_at::date as created, f.status, f.field_values ->> 'sellerName' as seller,
       coalesce(p.name, '(no property)') as property, coalesce(pe.display_name, '(no person)') as person
  from document_form_instance f
  left join property p on p.id = f.property_id
  left join person pe on pe.id = f.person_id
 where f.template_id = 'LISTING-01'
 order by f.created_at;

commit;
