-- CulebraLuxe Portal
-- FORMS-01 — form context is not universally a child of Deal.
--
-- A form instance may bind a Deal, a Person, a Property, or a combination.
-- At least one context pointer is required. Issued PDFs remain in the
-- existing DOC-06 transaction_document + media vault (DOCVAULT-01).

begin;

alter table document_form_instance
    alter column deal_id drop not null;

alter table document_form_instance
    add column if not exists person_id uuid references person(id) on delete set null,
    add column if not exists property_id uuid references property(id) on delete set null;

alter table document_form_instance
    drop constraint if exists document_form_instance_context_check;

alter table document_form_instance
    add constraint document_form_instance_context_check
    check (
        deal_id is not null
        or person_id is not null
        or property_id is not null
    );

create index if not exists idx_document_form_instance_person
    on document_form_instance(person_id);

create index if not exists idx_document_form_instance_property
    on document_form_instance(property_id);

-- Role-based participant collections for a draft (never buyer1/buyer2 columns).
create table if not exists document_form_participant (
    id uuid primary key default gen_random_uuid(),
    form_instance_id uuid not null
        references document_form_instance(id)
        on delete cascade,
    role text not null,
    person_id uuid references person(id) on delete set null,
    display_name text not null,
    sort_order integer not null default 0,
    created_at timestamptz not null default now()
);

create index if not exists idx_document_form_participant_form
    on document_form_participant(form_instance_id, role, sort_order);

-- Generated form artifacts may exist before a Deal is opened.
alter table transaction_document
    alter column deal_id drop not null;

alter table transaction_document
    drop constraint if exists transaction_document_context_check;

alter table transaction_document
    add constraint transaction_document_context_check
    check (deal_id is not null or party_person_id is not null);

commit;
