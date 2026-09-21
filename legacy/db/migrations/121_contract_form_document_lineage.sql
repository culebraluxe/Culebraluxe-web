-- CulebraLuxe
-- Canonical Contract lineage for mutable forms and immutable issued evidence.
-- Migration: 121_contract_form_document_lineage.sql
--
-- Contract is the agreement authority. Forms are mutable working context and
-- transaction_document is immutable issued evidence. New canonical paths carry
-- an explicit Contract FK; legacy Deal/Person context remains only so the small
-- set of real Listing agreements stays readable while the refactor proceeds.
--
-- Deliberately NO backfill: a Contract id is never guessed from a Person,
-- Property, Deal, template, or "latest" record.

begin;

alter table document_form_instance
    add column if not exists contract_id uuid
        references contract(id)
        on delete restrict;

create index if not exists idx_document_form_instance_contract
    on document_form_instance(contract_id)
    where contract_id is not null;

alter table document_form_instance
    drop constraint if exists document_form_instance_context_check;

alter table document_form_instance
    add constraint document_form_instance_context_check
    check (
        contract_id is not null
        or deal_id is not null
        or person_id is not null
        or property_id is not null
    );

alter table transaction_document
    add column if not exists contract_id uuid
        references contract(id)
        on delete restrict;

create index if not exists idx_transaction_document_contract
    on transaction_document(contract_id)
    where contract_id is not null;

create index if not exists idx_transaction_document_contract_state
    on transaction_document(contract_id, state)
    where contract_id is not null;

create index if not exists idx_transaction_document_contract_issued_lineage
    on transaction_document(contract_id, template_id, issued_version)
    where contract_id is not null and issued_version is not null;

alter table transaction_document
    drop constraint if exists transaction_document_context_check;

alter table transaction_document
    add constraint transaction_document_context_check
    check (
        contract_id is not null
        or deal_id is not null
        or party_person_id is not null
    );

comment on column document_form_instance.contract_id is
    'Explicit Contract context for canonical form work. Never inferred from Deal/Person/Property.';
comment on column transaction_document.contract_id is
    'Immutable lineage to the Contract this issued artifact evidences. Never inferred/backfilled heuristically.';

commit;
