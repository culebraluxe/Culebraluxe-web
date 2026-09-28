-- CulebraLuxe
-- Give engine tasks their canonical definition-node identity.
-- Migration: 145_tasks_node_identity.sql
--
-- WHY: `lib/agreements/crm26-consumer.ts` completes the Contract workflow's
-- P&S tasks by looking them up as
--     select id from tasks where process_instance_id = $1 and node_id = $2 ...
-- but the engine never wrote `node_id` (it wrote the node's display `name`), and
-- the column did not exist. The query therefore failed with 42703 against the
-- real database: contract execution could never complete `pns_executed`, so the
-- workflow never advanced past `pns_preparation`.
--
-- The engine now writes node.id on task creation; this column + index back it.

begin;

alter table tasks add column if not exists node_id text;

comment on column tasks.node_id is
    'Canonical workflow definition node id (node.id) that created this task. Written by WorkflowEngine on task creation; consumers look tasks up by (process_instance_id, node_id, status). Existing pre-migration rows keep null and are found by name/status instead.';

create index if not exists idx_tasks_instance_node_status
    on tasks (process_instance_id, node_id, status);

commit;
