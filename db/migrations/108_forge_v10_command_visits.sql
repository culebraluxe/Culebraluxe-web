-- ENG-FORGE-V10 — distinguish a replay of one command-node activation from a
-- later intentional visit after repair. Existing rows are visit 1.

alter table process_commands
  add column if not exists visit_sequence integer not null default 1;

alter table process_commands
  drop constraint if exists process_commands_process_instance_id_node_id_key;

create unique index if not exists process_commands_instance_node_visit_unique
  on process_commands (process_instance_id, node_id, visit_sequence);

create index if not exists idx_process_commands_instance_node
  on process_commands (process_instance_id, node_id, visit_sequence desc);
