-- PROJECTS-MVI-02: durable identity for the playbook instantiated by a Project.
-- Context ids are text because the existing Project/WBS seam is transport-neutral;
-- creation commands validate them when a canonical relationship is supplied.
alter table project add column if not exists project_type text;
alter table project add column if not exists playbook_id text;
alter table project add column if not exists playbook_version integer;
alter table project add column if not exists person_id text;
alter table project add column if not exists property_id text;
alter table project add column if not exists contract_id text;
create index if not exists project_playbook_idx on project (playbook_id, playbook_version);
create index if not exists project_context_property_idx on project (property_id);
