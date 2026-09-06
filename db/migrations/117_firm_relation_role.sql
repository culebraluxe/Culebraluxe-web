-- CulebraLuxe
-- P&S sidecar foundation: Firm + normalized relationship-role vocabulary.
-- Migration: 117_firm_relation_role.sql
--
-- IMPORTANT: `role` already means application authorization in this schema.
-- Business relationship roles therefore live in `relation_role`; this is the
-- canonical Role table for Person/Firm/Property/Contract relationship meaning.
--
-- Contract relationship tables are intentionally NOT created here. The service
-- kernel has ContractService, but this branch does not yet have canonical SQL
-- Contract persistence. Creating a second Deal/document-shaped contract table
-- just to satisfy the P&S sidecar would recreate the architecture we are trying
-- to remove. The sidecar models those edges at the Contract service seam until
-- canonical Contract persistence lands.

begin;

create table if not exists relation_role (
    id uuid primary key default gen_random_uuid(),
    scope text not null check (scope in (
        'person_person',
        'person_firm',
        'person_property',
        'firm_property',
        'contract_person',
        'contract_firm',
        'contract_property'
    )),
    code text not null,
    name text not null,
    description text,
    active boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint relation_role_scope_code_unique unique (scope, code),
    constraint relation_role_id_scope_unique unique (id, scope)
);

create table if not exists relation_role_alias (
    id uuid primary key default gen_random_uuid(),
    role_id uuid not null,
    scope text not null,
    alias text not null,
    normalized_alias text not null,
    created_at timestamptz not null default now(),
    constraint relation_role_alias_role_scope_fk
        foreign key (role_id, scope)
        references relation_role(id, scope)
        on delete cascade,
    constraint relation_role_alias_scope_alias_unique
        unique (scope, normalized_alias)
);

-- Firm is a durable noun: bank, brokerage, law firm, title/escrow company,
-- appraisal firm, LLC, etc. `kind` is intrinsic classification, never the
-- contextual role a Firm plays in a particular Contract.
create table if not exists firm (
    id uuid primary key default gen_random_uuid(),
    name text not null,
    legal_name text,
    kind text,
    status text not null default 'active',
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_firm_name on firm(lower(name));
create index if not exists idx_firm_legal_name on firm(lower(legal_name));

-- Person <-> Person: relationship meaning lives on the edge.
create table if not exists person_person (
    id uuid primary key default gen_random_uuid(),
    person_id uuid not null references person(id) on delete cascade,
    related_person_id uuid not null references person(id) on delete cascade,
    role_id uuid not null,
    role_scope text not null default 'person_person'
        check (role_scope = 'person_person'),
    status text not null default 'active',
    source_type text not null default 'manual',
    source_key text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint person_person_not_self check (person_id <> related_person_id),
    constraint person_person_role_scope_fk
        foreign key (role_id, role_scope)
        references relation_role(id, scope),
    constraint person_person_unique unique (person_id, related_person_id, role_id)
);

create index if not exists idx_person_person_person on person_person(person_id, role_id);
create index if not exists idx_person_person_related on person_person(related_person_id, role_id);

-- Person <-> Firm: job/professional context, not Person.role.
create table if not exists person_firm (
    id uuid primary key default gen_random_uuid(),
    person_id uuid not null references person(id) on delete cascade,
    firm_id uuid not null references firm(id) on delete cascade,
    role_id uuid not null,
    role_scope text not null default 'person_firm'
        check (role_scope = 'person_firm'),
    title text,
    license_number text,
    jurisdiction text,
    status text not null default 'active',
    start_date date,
    end_date date,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint person_firm_role_scope_fk
        foreign key (role_id, role_scope)
        references relation_role(id, scope),
    constraint person_firm_unique unique (person_id, firm_id, role_id)
);

create index if not exists idx_person_firm_person on person_firm(person_id, role_id);
create index if not exists idx_person_firm_firm on person_firm(firm_id, role_id);

-- Firm <-> Property: office/mailing/registered/ownership context. Property owns
-- the place/address facts; this mapping only says why that Property matters.
create table if not exists firm_property (
    id uuid primary key default gen_random_uuid(),
    firm_id uuid not null references firm(id) on delete cascade,
    property_id uuid not null references property(id) on delete cascade,
    role_id uuid not null,
    role_scope text not null default 'firm_property'
        check (role_scope = 'firm_property'),
    status text not null default 'active',
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint firm_property_role_scope_fk
        foreign key (role_id, role_scope)
        references relation_role(id, scope),
    constraint firm_property_unique unique (firm_id, property_id, role_id)
);

create index if not exists idx_firm_property_firm on firm_property(firm_id, role_id);
create index if not exists idx_firm_property_property on firm_property(property_id, role_id);

-- P&S proves these are intrinsic Property registry facts. They are additive;
-- PropertyService exposure is deliberately a separate seam from persistence.
alter table property add column if not exists registry_entry text;
alter table property add column if not exists finca_number text;
alter table property add column if not exists registry_section text;

-- Canonical vocabulary. New professional labels grow as rows, not new columns
-- or service methods.
insert into relation_role (scope, code, name, description) values
    ('person_person', 'SPOUSE', 'Spouse', 'Durable spouse relationship between two Persons.'),
    ('person_person', 'PARTNER', 'Partner', 'Durable partner relationship between two Persons.'),

    ('person_firm', 'BROKER', 'Broker', 'Person acts professionally as a broker for a Firm.'),
    ('person_firm', 'ATTORNEY', 'Attorney', 'Person acts professionally as an attorney for a Firm.'),
    ('person_firm', 'NOTARY', 'Notary', 'Person acts professionally as a notary for a Firm.'),
    ('person_firm', 'LOAN_OFFICER', 'Loan Officer', 'Person acts as a loan officer for a Firm.'),
    ('person_firm', 'APPRAISER', 'Appraiser', 'Person acts professionally as an appraiser for a Firm.'),
    ('person_firm', 'SURVEYOR', 'Surveyor', 'Person acts professionally as a surveyor for a Firm.'),
    ('person_firm', 'ESCROW_OFFICER', 'Escrow Officer', 'Person acts as an escrow officer for a Firm.'),

    ('person_property', 'ADDRESS', 'Address', 'Property is an address relevant to a Person.'),
    ('person_property', 'LEGAL_ADDRESS', 'Legal Address', 'Property is a legal/mailing address for a Person.'),
    ('person_property', 'PHYSICAL_PROPERTY', 'Physical Property', 'Property is a physical real-estate context for a Person.'),
    ('person_property', 'INTEREST', 'Interest', 'Person has an interest relationship to a Property.'),

    ('firm_property', 'OFFICE', 'Office', 'Property is an office location for a Firm.'),
    ('firm_property', 'MAILING', 'Mailing', 'Property is a mailing location for a Firm.'),
    ('firm_property', 'REGISTERED', 'Registered', 'Property is a registered/legal address for a Firm.'),
    ('firm_property', 'OWNER', 'Owner', 'Firm is an owner of the Property.'),

    ('contract_person', 'BUYER', 'Buyer', 'Person participates in the Contract as Buyer.'),
    ('contract_person', 'SELLER', 'Seller', 'Person participates in the Contract as Seller.'),
    ('contract_person', 'BUYER_BROKER', 'Buyer Broker', 'Person is the Buyer-side broker on the Contract.'),
    ('contract_person', 'SELLER_BROKER', 'Seller Broker', 'Person is the Seller-side broker on the Contract.'),
    ('contract_person', 'SELLER_SPOUSE', 'Seller Spouse', 'Person joins this Contract as the Seller spouse.'),
    ('contract_person', 'CLOSING_NOTARY', 'Closing Notary', 'Person is the closing notary for this Contract.'),
    ('contract_person', 'LENDER_CONTACT', 'Lender Contact', 'Person is the lender contact for this Contract.'),
    ('contract_person', 'BUYER_COUNSEL', 'Buyer Counsel', 'Person is counsel to Buyer on this Contract.'),
    ('contract_person', 'SELLER_COUNSEL', 'Seller Counsel', 'Person is counsel to Seller on this Contract.'),

    ('contract_firm', 'BUYER', 'Buyer', 'Firm/legal entity participates in the Contract as Buyer.'),
    ('contract_firm', 'SELLER', 'Seller', 'Firm/legal entity participates in the Contract as Seller.'),
    ('contract_firm', 'BUYER_BROKERAGE', 'Buyer Brokerage', 'Firm is the Buyer-side brokerage on the Contract.'),
    ('contract_firm', 'SELLER_BROKERAGE', 'Seller Brokerage', 'Firm is the Seller-side brokerage on the Contract.'),
    ('contract_firm', 'LENDER', 'Lender', 'Firm is the lender on the Contract.'),
    ('contract_firm', 'ESCROW_HOLDER', 'Escrow Holder', 'Firm holds escrow for the Contract.'),
    ('contract_firm', 'TITLE_COMPANY', 'Title Company', 'Firm provides title services for the Contract.'),
    ('contract_firm', 'LAW_FIRM', 'Law Firm', 'Firm provides legal services for the Contract.'),
    ('contract_firm', 'APPRAISAL_FIRM', 'Appraisal Firm', 'Firm provides appraisal services for the Contract.'),

    ('contract_property', 'SUBJECT_PROPERTY', 'Subject Property', 'Property is the subject of the Contract.')
on conflict (scope, code) do nothing;

-- Alias resolution is scoped. "Counsel" is not globally rewritten to
-- ATTORNEY; the same language may mean a different Contract relationship.
insert into relation_role_alias (role_id, scope, alias, normalized_alias)
select r.id, r.scope, a.alias, lower(regexp_replace(trim(a.alias), '\s+', ' ', 'g'))
from relation_role r
join (values
    ('person_firm', 'ATTORNEY', 'lawyer'),
    ('person_firm', 'ATTORNEY', 'attorney'),
    ('person_firm', 'ATTORNEY', 'abogado'),
    ('person_firm', 'BROKER', 'real estate broker'),
    ('person_firm', 'LOAN_OFFICER', 'banker'),
    ('contract_person', 'BUYER_COUNSEL', 'buyer lawyer'),
    ('contract_person', 'BUYER_COUNSEL', 'buyer attorney'),
    ('contract_person', 'BUYER_COUNSEL', 'buyer counsel'),
    ('contract_person', 'SELLER_COUNSEL', 'seller lawyer'),
    ('contract_person', 'SELLER_COUNSEL', 'seller attorney'),
    ('contract_person', 'SELLER_COUNSEL', 'seller counsel'),
    ('contract_firm', 'LENDER', 'bank'),
    ('contract_firm', 'ESCROW_HOLDER', 'escrow company')
) as a(scope, code, alias)
  on r.scope = a.scope and r.code = a.code
on conflict (scope, normalized_alias) do nothing;

commit;
