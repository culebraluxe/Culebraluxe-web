-- CulebraLuxe Native Document Signing Foundation
-- File: docsign.sql
--
-- Purpose:
--   Database foundation for the native Rust document-signing stack:
--     - DocumentSignService
--     - SignerService
--     - EmailService
--
-- Architectural prerequisites already present in CulebraLuxe:
--   - signature_request                    (migration 036)
--   - signature_envelope_recipient         (migration 092)
--   - transaction_document / media / Vault
--   - workflow command receipts
--   - transactional outbox / MQ
--   - Casbin authorization / entitlements
--
-- Important:
--   * signature_request remains the ONLY canonical envelope lifecycle.
--   * document_sign_request stores native-provider/configuration data only.
--   * Vault remains the ONLY service boundary for document bytes.
--   * Existing outbox/MQ is reused; this file creates no second queue.
--   * Advanced cryptographic sealing / PKI is intentionally deferred.
--
-- Safe to run before the Rust services are implemented.
-- Designed to be idempotent for normal re-runs and partial prior execution.

begin;

-- ============================================================================
-- 1. Native DocumentSign provider/configuration record
-- ============================================================================
-- There is deliberately NO status column here.
-- Canonical status remains signature_request.status.

create table if not exists document_sign_request (
    signature_request_id uuid primary key
        references signature_request(id)
        on delete cascade,

    subject text
        check (subject is null or char_length(subject) <= 500),

    signing_mode text not null default 'sequential'
        check (signing_mode in ('sequential', 'parallel')),

    expires_at timestamptz,
    issued_at timestamptz,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint document_sign_request_expiry_after_create
        check (expires_at is null or expires_at > created_at),

    constraint document_sign_request_issue_after_create
        check (issued_at is null or issued_at >= created_at)
);

create index if not exists idx_document_sign_request_expires
    on document_sign_request(expires_at)
    where expires_at is not null;

create index if not exists idx_document_sign_request_issued
    on document_sign_request(issued_at)
    where issued_at is not null;


-- ============================================================================
-- 2. Extend the existing immutable issued-recipient snapshot
-- ============================================================================
-- signer_order:
--   deterministic unique recipient position already owned by CulebraLuxe.
--
-- signing_step:
--   execution group. Recipients with the same step may act in parallel.
--
-- Example:
--   A signer_order=1, signing_step=1
--   B signer_order=2, signing_step=1
--   C signer_order=3, signing_step=2
--
-- A+B may act together, then C.

alter table signature_envelope_recipient
    add column if not exists recipient_role text;

update signature_envelope_recipient
set recipient_role = 'signer'
where recipient_role is null
   or btrim(recipient_role) = '';

alter table signature_envelope_recipient
    alter column recipient_role set default 'signer';

alter table signature_envelope_recipient
    alter column recipient_role set not null;

do $$
begin
    if not exists (
        select 1
        from pg_constraint
        where conname = 'signature_envelope_recipient_role_check'
          and conrelid = 'signature_envelope_recipient'::regclass
    ) then
        alter table signature_envelope_recipient
            add constraint signature_envelope_recipient_role_check
            check (recipient_role in ('signer', 'approver'));
    end if;
end
$$;

alter table signature_envelope_recipient
    add column if not exists signing_step integer;

update signature_envelope_recipient
set signing_step = signer_order
where signing_step is null;

alter table signature_envelope_recipient
    alter column signing_step set not null;

do $$
begin
    if not exists (
        select 1
        from pg_constraint
        where conname = 'signature_envelope_recipient_signing_step_positive'
          and conrelid = 'signature_envelope_recipient'::regclass
    ) then
        alter table signature_envelope_recipient
            add constraint signature_envelope_recipient_signing_step_positive
            check (signing_step > 0);
    end if;
end
$$;

create index if not exists idx_signature_envelope_recipient_step
    on signature_envelope_recipient(
        signature_request_id,
        signing_step,
        signer_order
    );

-- Supports composite foreign keys below and makes envelope ownership structural.
create unique index if not exists uq_signature_envelope_recipient_request_id_pair
    on signature_envelope_recipient(signature_request_id, id);


-- ============================================================================
-- 3. Per-recipient execution state
-- ============================================================================
-- Envelope lifecycle is NOT stored here.
-- This is human-recipient execution state only.

create table if not exists signature_recipient_state (
    recipient_id uuid primary key
        references signature_envelope_recipient(id)
        on delete cascade,

    state text not null default 'pending'
        check (state in (
            'pending',
            'notified',
            'viewed',
            'in_progress',
            'completed',
            'declined',
            'expired',
            'revoked'
        )),

    notified_at timestamptz,
    first_viewed_at timestamptz,
    completed_at timestamptz,
    declined_at timestamptz,
    expired_at timestamptz,
    last_activity_at timestamptz,

    revision bigint not null default 0
        check (revision >= 0),

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_signature_recipient_state_state
    on signature_recipient_state(state);

create index if not exists idx_signature_recipient_state_activity
    on signature_recipient_state(last_activity_at)
    where last_activity_at is not null;


-- ============================================================================
-- 4. Recipient-owned signing fields
-- ============================================================================
-- Coordinates are percentages of the PDF page, not pixels.
--
-- Critical structural invariant:
--   (signature_request_id, recipient_id) must identify a recipient belonging
--   to THIS SAME signature request.
--
-- Application code must still enforce draft/issued mutability rules.

create table if not exists signature_field (
    id uuid primary key default gen_random_uuid(),

    signature_request_id uuid not null
        references signature_request(id)
        on delete cascade,

    recipient_id uuid not null,

    field_key text not null,

    field_type text not null
        check (field_type in (
            'signature',
            'initials',
            'name',
            'email',
            'date',
            'text',
            'number',
            'checkbox',
            'radio',
            'dropdown'
        )),

    page_number integer not null
        check (page_number > 0),

    position_x numeric(7,4) not null
        check (position_x >= 0 and position_x <= 100),

    position_y numeric(7,4) not null
        check (position_y >= 0 and position_y <= 100),

    width numeric(7,4) not null
        check (width > 0 and width <= 100),

    height numeric(7,4) not null
        check (height > 0 and height <= 100),

    required boolean not null default true,

    label text
        check (label is null or btrim(label) <> ''),

    configuration jsonb not null default '{}'::jsonb
        check (jsonb_typeof(configuration) = 'object'),

    created_at timestamptz not null default now(),

    constraint signature_field_key_not_blank
        check (btrim(field_key) <> ''),

    constraint signature_field_horizontal_bounds
        check (position_x + width <= 100),

    constraint signature_field_vertical_bounds
        check (position_y + height <= 100),

    constraint signature_field_request_recipient_fk
        foreign key (signature_request_id, recipient_id)
        references signature_envelope_recipient(signature_request_id, id)
        on delete cascade,

    constraint signature_field_request_key_unique
        unique (signature_request_id, field_key),

    -- Supports the response table's ownership-enforcing composite FK.
    constraint signature_field_id_recipient_unique
        unique (id, recipient_id)
);

create index if not exists idx_signature_field_request
    on signature_field(signature_request_id, page_number, id);

create index if not exists idx_signature_field_recipient
    on signature_field(recipient_id, page_number, id);


-- ============================================================================
-- 5. Field responses
-- ============================================================================
-- One response per field.
--
-- Composite FK guarantees the responding recipient is the field owner.

create table if not exists signature_field_response (
    field_id uuid primary key,

    recipient_id uuid not null,

    value jsonb not null,

    completed_at timestamptz not null default now(),

    constraint signature_field_response_owner_fk
        foreign key (field_id, recipient_id)
        references signature_field(id, recipient_id)
        on delete cascade
);

create index if not exists idx_signature_field_response_recipient
    on signature_field_response(recipient_id, completed_at);


-- ============================================================================
-- 6. Recipient signing access
-- ============================================================================
-- No plaintext signing secret is stored.
--
-- The future Rust signer-access token carries:
--   access_id / recipient_id / token_version / expiry
-- and is authenticated by the application.
--
-- This DB row controls revocation, expiry and token rotation.

create table if not exists signature_recipient_access (
    id uuid primary key default gen_random_uuid(),

    recipient_id uuid not null unique
        references signature_envelope_recipient(id)
        on delete cascade,

    token_version integer not null default 1
        check (token_version > 0),

    expires_at timestamptz not null,
    revoked_at timestamptz,
    last_used_at timestamptz,

    created_at timestamptz not null default now(),

    constraint signature_recipient_access_expiry_after_create
        check (expires_at > created_at),

    constraint signature_recipient_access_revoked_after_create
        check (revoked_at is null or revoked_at >= created_at)
);

create index if not exists idx_signature_recipient_access_active_expiry
    on signature_recipient_access(expires_at)
    where revoked_at is null;


-- ============================================================================
-- 7. Exact write-once consent evidence
-- ============================================================================
-- Service/repository code must expose INSERT/read only.
-- Exact displayed consent text is retained together with its SHA-256.

create table if not exists signature_recipient_consent (
    recipient_id uuid primary key
        references signature_envelope_recipient(id)
        on delete cascade,

    consent_version text not null
        check (btrim(consent_version) <> ''),

    consent_text text not null
        check (btrim(consent_text) <> ''),

    consent_text_sha256 text not null
        check (consent_text_sha256 ~ '^[0-9a-f]{64}$'),

    accepted_at timestamptz not null,

    ip_address inet,
    user_agent text
);


-- ============================================================================
-- 8. Append-oriented signer evidence
-- ============================================================================
-- Service/repository API must expose append/read operations only.
--
-- recipient_id is nullable for envelope-level events, but when present the
-- composite FK guarantees that the recipient belongs to the same envelope.

create table if not exists signature_evidence_event (
    id uuid primary key default gen_random_uuid(),

    signature_request_id uuid not null
        references signature_request(id)
        on delete cascade,

    recipient_id uuid,

    event_type text not null
        check (btrim(event_type) <> ''),

    occurred_at timestamptz not null default now(),

    actor_id text,
    correlation_id text,
    causation_id text,

    evidence jsonb not null default '{}'::jsonb
        check (jsonb_typeof(evidence) = 'object'),

    constraint signature_evidence_request_recipient_fk
        foreign key (signature_request_id, recipient_id)
        references signature_envelope_recipient(signature_request_id, id)
        on delete restrict
);

create index if not exists idx_signature_evidence_request
    on signature_evidence_event(signature_request_id, occurred_at, id);

create index if not exists idx_signature_evidence_recipient
    on signature_evidence_event(recipient_id, occurred_at, id)
    where recipient_id is not null;

create index if not exists idx_signature_evidence_type
    on signature_evidence_event(event_type, occurred_at);


-- ============================================================================
-- 9. Durable transactional email state
-- ============================================================================
-- This is NOT a second queue.
--
-- EmailService.queue writes this durable business record, then emits
-- `email.delivery.requested` through CulebraLuxe's EXISTING transactional
-- outbox/MQ. The worker uses the existing MQ lease/retry/dead-letter behavior.

create table if not exists email_message (
    id uuid primary key default gen_random_uuid(),

    message_kind text not null
        check (message_kind in (
            'signature_invitation',
            'signature_reminder',
            'signature_completed',
            'signature_declined'
        )),

    recipient_email text not null
        check (btrim(recipient_email) <> ''),

    template_key text not null
        check (btrim(template_key) <> ''),

    template_payload jsonb not null default '{}'::jsonb
        check (jsonb_typeof(template_payload) = 'object'),

    dedupe_key text not null unique
        check (btrim(dedupe_key) <> ''),

    status text not null default 'queued'
        check (status in (
            'queued',
            'sending',
            'sent',
            'failed',
            'dead',
            'cancelled'
        )),

    provider_message_id text,

    attempt_count integer not null default 0
        check (attempt_count >= 0),

    last_error text,

    correlation_id text,
    causation_id text,

    queued_at timestamptz not null default now(),
    sent_at timestamptz,
    updated_at timestamptz not null default now()
);

create index if not exists idx_email_message_delivery_queue
    on email_message(status, queued_at, id)
    where status in ('queued', 'failed');

create index if not exists idx_email_message_recipient
    on email_message(recipient_email, queued_at desc);

create index if not exists idx_email_message_correlation
    on email_message(correlation_id)
    where correlation_id is not null;


-- ============================================================================
-- 10. updated_at maintenance
-- ============================================================================
-- Small DB backstop for the new mutable runtime tables.

create or replace function docsign_touch_updated_at()
returns trigger
language plpgsql
as $$
begin
    new.updated_at = now();
    return new;
end;
$$;

drop trigger if exists trg_document_sign_request_touch_updated_at
    on document_sign_request;

create trigger trg_document_sign_request_touch_updated_at
before update on document_sign_request
for each row
execute function docsign_touch_updated_at();


drop trigger if exists trg_signature_recipient_state_touch_updated_at
    on signature_recipient_state;

create trigger trg_signature_recipient_state_touch_updated_at
before update on signature_recipient_state
for each row
execute function docsign_touch_updated_at();


drop trigger if exists trg_email_message_touch_updated_at
    on email_message;

create trigger trg_email_message_touch_updated_at
before update on email_message
for each row
execute function docsign_touch_updated_at();


-- ============================================================================
-- 11. Authorization / entitlement note
-- ============================================================================
-- DO NOT seed the new DocumentSign/Signer/Email action grants in this SQL yet.
--
-- The current Rust CasbinAuthorizationPort only recognizes actions declared in:
--   rust/server/src/security/entitlement_catalog.rs
--
-- The implementation pass must first add the new action codes to that canonical
-- Rust catalog, then seed role_entitlement grants using the normal Security
-- service/entitlement path. Creating DB grants before the action catalog exists
-- would make the database LOOK authorized while the running application still
-- correctly refuses the unknown actions.
--
-- External signer commands additionally require an explicit narrow system/
-- signer-capability rule in Casbin; they must NOT be opened through a generic
-- anonymous/GUEST command grant.


commit;


-- ============================================================================
-- Verification
-- ============================================================================
-- These reads do not mutate anything. Running the whole file in a SQL editor
-- should return each expected table name when installation succeeded.

select to_regclass('public.document_sign_request')      as document_sign_request;
select to_regclass('public.signature_recipient_state')  as signature_recipient_state;
select to_regclass('public.signature_field')            as signature_field;
select to_regclass('public.signature_field_response')   as signature_field_response;
select to_regclass('public.signature_recipient_access') as signature_recipient_access;
select to_regclass('public.signature_recipient_consent') as signature_recipient_consent;
select to_regclass('public.signature_evidence_event')   as signature_evidence_event;
select to_regclass('public.email_message')              as email_message;

select
    column_name,
    data_type,
    is_nullable
from information_schema.columns
where table_schema = 'public'
  and table_name = 'signature_envelope_recipient'
  and column_name in ('recipient_role', 'signing_step')
order by column_name;
