-- Stable, project-scoped WBS links. The service serializes writes per project
-- and rejects cycles before insertion. The composite FKs enforce scope in SQL.
create unique index if not exists wbs_item_project_id_id_unique on wbs_item(project_id, id);

create table if not exists wbs_dependency (
    project_id text not null references project(id) on delete cascade,
    source_id text not null,
    target_id text not null,
    kind text not null default 'finish_to_start' check (kind = 'finish_to_start'),
    primary key (source_id, target_id),
    constraint wbs_dependency_distinct check (source_id <> target_id),
    foreign key (project_id, source_id) references wbs_item(project_id, id) on delete cascade,
    foreign key (project_id, target_id) references wbs_item(project_id, id) on delete cascade
);
create index if not exists wbs_dependency_project_target on wbs_dependency(project_id, target_id);
