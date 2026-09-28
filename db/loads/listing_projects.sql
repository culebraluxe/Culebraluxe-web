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

begin;

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

commit;
