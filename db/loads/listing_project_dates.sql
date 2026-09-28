-- LISTING PROJECT DATES — each listing project's six steps dated from what the database shows, so its Gantt tells the
-- listing's real history. Only a step with no planned dates yet is filled: a date set by hand is never overwritten.
-- Safe to run again.
--
--   Listing Contract Created        the property's issued listing contract (LISTING-01), created on    -> done
--   Listing Contract Signed         that contract's issue date, as a PLAN — no signature is recorded    -> left open
--   Property Media Shot             the first photo's upload                                             -> done
--   Property Listing Added to Website  when the property record was created                             -> done
--   Property Media Uploaded to Listing  first to last photo upload                                       -> done
--   QA: Property Listing Live in PROD   the later of the listing and the last photo                     -> done
--
-- A step with no evidence stays open and undated — the contracts still to chase.

begin;

create temporary table listing_evidence on commit drop as
select pj.id as project_id,
       p.created_at::date as listed,
       (select min(m.created_at)::date from property_media pm join media m on m.id = pm.media_id
         where pm.property_id = p.id and m.media_type = 'image' and m.derivative_of is null) as first_photo,
       (select max(m.created_at)::date from property_media pm join media m on m.id = pm.media_id
         where pm.property_id = p.id and m.media_type = 'image' and m.derivative_of is null) as last_photo,
       (select min(f.created_at)::date from document_form_instance f
         where f.property_id = p.id and f.template_id = 'LISTING-01' and f.status <> 'draft') as contract_created,
       (select min(f.updated_at)::date from document_form_instance f
         where f.property_id = p.id and f.template_id = 'LISTING-01' and f.status <> 'draft') as contract_issued
  from project pj
  join property p on p.id::text = pj.property_id
 where pj.project_type = 'listing';

create temporary table listing_step_dates on commit drop as
select e.project_id || '-' || s.suffix as item_id, s.start_date, s.finish_date, s.done
  from listing_evidence e
 cross join lateral (values
     ('contract-created', e.contract_created, e.contract_created, true),
     ('contract-signed',  e.contract_issued,  e.contract_issued,  false),
     ('media-shot',       e.first_photo,      e.first_photo,      true),
     ('website-added',    e.listed,           e.listed,           true),
     ('media-uploaded',   e.first_photo,      e.last_photo,       true),
     ('qa-live',          greatest(e.listed, e.last_photo), greatest(e.listed, e.last_photo), true)
 ) as s(suffix, start_date, finish_date, done)
 where s.start_date is not null;

with dated as (
    update wbs_item w
       set planned_start = d.start_date,
           planned_finish = greatest(d.start_date, d.finish_date),
           status = case when d.done and w.status = 'open' then 'done' else w.status end,
           updated_at = now()
      from listing_step_dates d
     where w.id = d.item_id
       and w.planned_start is null and w.planned_finish is null
    returning w.project_id, w.title, w.planned_start, w.planned_finish, w.status
)
select pj.name as project, dated.title as step, dated.planned_start, dated.planned_finish, dated.status
  from dated join project pj on pj.id = dated.project_id
 order by pj.name, dated.planned_start, dated.title;

-- What is still undated: the steps to chase.
select pj.name as project, w.title as still_undated
  from wbs_item w join project pj on pj.id = w.project_id
 where pj.project_type = 'listing' and w.planned_start is null
 order by pj.name, w.sort_order;

commit;
