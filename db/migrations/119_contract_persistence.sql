-- CulebraLuxe
-- Canonical Contract persistence: agreement truth + normalized Role mappings.
-- Migration: 119_contract_persistence.sql
--
-- Contract owns agreement-scoped facts and lineage.
-- Person/Firm/Property remain independent canonical identities.
-- Role is the normalized contextual position mapped onto those identities.

begin;

create table if not exists contract (
    id uuid primary key,
    contract_type text not null,
    form_template_id text not null,
    source_form_instance_id uuid,
    predecessor_contract_id uuid
        references contract(id)
        on delete restrict,
    facts jsonb not null default '{}'::jsonb,
    status text not null default 'draft',
    executed_at timestamptz,
    evidence_document_id uuid,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint contract_not_own_predecessor
        check (predecessor_contract_id is null or predecessor_contract_id <> id)
);

create index if not exists idx_contract_type_status
    on contract(contract_type, status);
create index if not exists idx_contract_predecessor
    on contract(predecessor_contract_id)
    where predecessor_contract_id is not null;
create index if not exists idx_contract_source_form
    on contract(source_form_instance_id)
    where source_form_instance_id is not null;

-- Person -> Role(position) in this Contract.
create table if not exists contract_person (
    id uuid primary key default gen_random_uuid(),
    contract_id uuid not null
        references contract(id)
        on delete cascade,
    person_id uuid not null
        references person(id)
        on delete restrict,
    role_id uuid not null,
    role_scope text not null default 'contract_person'
        check (role_scope = 'contract_person'),
    ordinal integer not null default 0
        check (ordinal >= 0),
    snapshot_name text,
    attributes jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint contract_person_role_scope_fk
        foreign key (role_id, role_scope)
        references role(id, scope),
    constraint contract_person_assignment_unique
        unique (contract_id, role_id, person_id, ordinal)
);

create index if not exists idx_contract_person_contract_role
    on contract_person(contract_id, role_id, ordinal);
create index if not exists idx_contract_person_person
    on contract_person(person_id, role_id);

-- Firm -> Role(position) in this Contract.
create table if not exists contract_firm (
    id uuid primary key default gen_random_uuid(),
    contract_id uuid not null
        references contract(id)
        on delete cascade,
    firm_id uuid not null
        references firm(id)
        on delete restrict,
    role_id uuid not null,
    role_scope text not null default 'contract_firm'
        check (role_scope = 'contract_firm'),
    ordinal integer not null default 0
        check (ordinal >= 0),
    snapshot_name text,
    attributes jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint contract_firm_role_scope_fk
        foreign key (role_id, role_scope)
        references role(id, scope),
    constraint contract_firm_assignment_unique
        unique (contract_id, role_id, firm_id, ordinal)
);

create index if not exists idx_contract_firm_contract_role
    on contract_firm(contract_id, role_id, ordinal);
create index if not exists idx_contract_firm_firm
    on contract_firm(firm_id, role_id);

-- Contract -> Property uses the same Role vocabulary. Today SUBJECT_PROPERTY is
-- the primary use; the mapping preserves the normalized edge rather than
-- duplicating a property_id column on Contract.
create table if not exists contract_property (
    id uuid primary key default gen_random_uuid(),
    contract_id uuid not null
        references contract(id)
        on delete cascade,
    property_id uuid not null
        references property(id)
        on delete restrict,
    role_id uuid not null,
    role_scope text not null default 'contract_property'
        check (role_scope = 'contract_property'),
    ordinal integer not null default 0
        check (ordinal >= 0),
    snapshot_name text,
    attributes jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint contract_property_role_scope_fk
        foreign key (role_id, role_scope)
        references role(id, scope),
    constraint contract_property_assignment_unique
        unique (contract_id, role_id, ordinal)
);

create index if not exists idx_contract_property_contract_role
    on contract_property(contract_id, role_id, ordinal);
create index if not exists idx_contract_property_property
    on contract_property(property_id, role_id);

comment on table contract is
    'Canonical durable agreement truth. Workflow owns lifecycle orchestration; Deal is not canonical Contract truth.';
comment on table contract_person is
    'Person identity mapped to a normalized Role(position) within one Contract.';
comment on table contract_firm is
    'Firm identity mapped to a normalized Role(position) within one Contract.';
comment on table contract_property is
    'Property identity mapped to a normalized Contract Role such as SUBJECT_PROPERTY.';

commit;
