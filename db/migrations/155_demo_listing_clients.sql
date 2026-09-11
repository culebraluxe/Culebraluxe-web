-- CulebraLuxe
-- One stand-in client per stand-in listing (temporary).
-- Migration: 155_demo_listing_clients.sql
--
-- Migration 154 assigned ONE shared placeholder person to all five stand-in
-- listings. A listing has its own client, so the stand-ins get their own too —
-- this keeps the model honest while the demo site is still showing them.
--
-- These are temporary. `display_name_source = 'demo'` marks every one of them, so
-- they are found (and removed) with a single predicate, together with the
-- `source_type = 'demo'` properties they own.
--
-- Nothing here changes is_published: the stand-in listings keep showing on the
-- public site exactly as they do today.

begin;

insert into person (id, display_name, display_name_source, role, status) values
    ('deadbeef-0000-4000-8000-000000000002', 'DEMO — Brisas del Mar (stand-in client)', 'demo', 'unclassified', 'new'),
    ('deadbeef-0000-4000-8000-000000000003', 'DEMO — Casa Brisa (stand-in client)', 'demo', 'unclassified', 'new'),
    ('deadbeef-0000-4000-8000-000000000004', 'DEMO — Casa Horizonte (stand-in client)', 'demo', 'unclassified', 'new'),
    ('deadbeef-0000-4000-8000-000000000005', 'DEMO — Casa Solana (stand-in client)', 'demo', 'unclassified', 'new'),
    ('deadbeef-0000-4000-8000-000000000006', 'DEMO — Sunset Point (stand-in client)', 'demo', 'unclassified', 'new')
on conflict (id) do nothing;

update property p
   set seller_person_id = v.person_id
  from (values
    ('CL-DEMO-002', 'deadbeef-0000-4000-8000-000000000005'::uuid),
    ('CL-DEMO-003', 'deadbeef-0000-4000-8000-000000000006'::uuid),
    ('CL-DEMO-004', 'deadbeef-0000-4000-8000-000000000002'::uuid),
    ('CL-DEMO-005', 'deadbeef-0000-4000-8000-000000000003'::uuid),
    ('CL-DEMO-006', 'deadbeef-0000-4000-8000-000000000004'::uuid)
  ) as v(listing_identifier, person_id)
 where p.listing_identifier = v.listing_identifier;

-- The shared stand-in from 154 is now unreferenced; remove it.
delete from person
 where id = 'deadbeef-0000-4000-8000-000000000001'
   and not exists (select 1 from property where seller_person_id = person.id);

commit;
