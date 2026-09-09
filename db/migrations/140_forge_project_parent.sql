-- Forge/Projects wiring: make the Project service's `project` table the
-- canonical parent of WBS work items. Today wbs_item.project_id points at the
-- WBS service's own `wbs_project` table, so the two services were never truly
-- linked. Repoint the FK to `project` (treat wbs_project as obsolete; the table
-- is left in place, unused).
-- Migration: 140_forge_project_parent.sql

begin;

alter table wbs_item drop constraint if exists wbs_item_project_id_fkey;

-- Remove orphaned items (their project_id refers to a wbs_project row that is
-- not a real `project`, e.g. earlier DEV test seeds). Only rows whose parent no
-- longer exists as a real Project are removed; real WBS data under a Project is
-- untouched.
delete from wbs_item w
 where w.project_id is not null
   and not exists (select 1 from project p where p.id = w.project_id);

alter table wbs_item
  add constraint wbs_item_project_id_fkey
  foreign key (project_id) references project(id) on delete cascade;

commit;
