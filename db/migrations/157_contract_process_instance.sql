-- CulebraLuxe
-- The cord: a Contract names the process instance (transaction) it belongs to.
-- Migration: 157_contract_process_instance.sql
--
-- Why (docs/REAL-ESTATE-TRANSACTION-DESIGN.md, sections 5.1 and 7.2):
--
--   process_instances.subject_type='deal' + subject_id=<deal id>   (the transaction)
--   contract.???                                                   (nothing)
--
-- Nothing on `contract` referenced the process that produces it, so the
-- artifacts and the transaction could not see each other. This column is that
-- link, and both directions become a plain query:
--
--   which contracts does this transaction have -> where process_instance_id = ?
--   which transaction does this contract belong to -> contract.process_instance_id
--
-- Nullable on purpose: a contract may exist with no process at all (a showing
-- report that goes nowhere). No cascade: an executed instrument is evidence and
-- is never deleted with its process (design section 9.4).

begin;

alter table contract add column if not exists process_instance_id uuid;

comment on column contract.process_instance_id is
    'The workflow/process instance (transaction) this Contract belongs to — the cord between artifacts and the process that produces them. Nullable: a Contract may exist with no process. ON DELETE SET NULL: a Contract is never deleted with its process.';

alter table contract drop constraint if exists contract_process_instance_id_fkey;

alter table contract
    add constraint contract_process_instance_id_fkey
    foreign key (process_instance_id) references process_instances (id) on delete set null;

create index if not exists contract_process_instance_idx
    on contract (process_instance_id)
    where process_instance_id is not null;

commit;
