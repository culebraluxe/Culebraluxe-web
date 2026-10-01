-- 260 — Native CulebraLuxe document-signing foundation.
--
-- Canonical copy of the idempotent docsign.sql applied manually to DEV and PROD
-- before the Rust services landed. Re-running is safe; future environments receive
-- the same schema through normal migration history.
--
-- signature_request remains the only canonical envelope lifecycle.
-- Vault remains the only document-byte authorization boundary.
-- Existing outbox/MQ remains the delivery queue.

begin;

create table if not exists document_sign_request (
    signature_request_id uuid primary key
        references signature_request(id) on delete cascade,
    subject text check (subject is null or char_length(subject) <= 500),
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

alter table signature_envelope_recipient
    add column if not exists recipient_role text;
update signature_envelope_recipient
   set recipient_role = 'signer'
 where recipient_role is null or btrim(recipient_role) = '';
alter table signature_envelope_recipient
    alter column recipient_role set default 'signer';
alter table signature_envelope_recipient
    alter column recipient_role set not null;

do $$
begin
    if not exists (
        select 1 from pg_constraint
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
        select 1 from pg_constraint
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
    on signature_envelope_recipient(signature_request_id, signing_step, signer_order);
create unique index if not exists uq_signature_envelope_recipient_request_id_pair
    on signature_envelope_recipient(signature_request_id, id);

create table if not exists signature_recipient_state (
    recipient_id uuid primary key
        references signature_envelope_recipient(id) on delete cascade,
    state text not null default 'pending'
        check (state in (
            'pending','notified','viewed','in_progress',
            'completed','declined','expired','revoked'
        )),
    notified_at timestamptz,
    first_viewed_at timestamptz,
    completed_at timestamptz,
    declined_at timestamptz,
    expired_at timestamptz,
    last_activity_at timestamptz,
    revision bigint not null default 0 check (revision >= 0),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
create index if not exists idx_signature_recipient_state_state
    on signature_recipient_state(state);
create index if not exists idx_signature_recipient_state_activity
    on signature_recipient_state(last_activity_at)
    where last_activity_at is not null;

create table if not exists signature_field (
    id uuid primary key default gen_random_uuid(),
    signature_request_id uuid not null
        references signature_request(id) on delete cascade,
    recipient_id uuid not null,
    field_key text not null,
    field_type text not null
        check (field_type in (
            'signature','initials','name','email','date','text',
            'number','checkbox','radio','dropdown'
        )),
    page_number integer not null check (page_number > 0),
    position_x numeric(7,4) not null check (position_x >= 0 and position_x <= 100),
    position_y numeric(7,4) not null check (position_y >= 0 and position_y <= 100),
    width numeric(7,4) not null check (width > 0 and width <= 100),
    height numeric(7,4) not null check (height > 0 and height <= 100),
    required boolean not null default true,
    label text check (label is null or btrim(label) <> ''),
    configuration jsonb not null default '{}'::jsonb
        check (jsonb_typeof(configuration) = 'object'),
    created_at timestamptz not null default now(),
    constraint signature_field_key_not_blank check (btrim(field_key) <> ''),
    constraint signature_field_horizontal_bounds check (position_x + width <= 100),
    constraint signature_field_vertical_bounds check (position_y + height <= 100),
    constraint signature_field_request_recipient_fk
        foreign key (signature_request_id, recipient_id)
        references signature_envelope_recipient(signature_request_id, id)
        on delete cascade,
    constraint signature_field_request_key_unique
        unique (signature_request_id, field_key),
    constraint signature_field_id_recipient_unique
        unique (id, recipient_id)
);
create index if not exists idx_signature_field_request
    on signature_field(signature_request_id, page_number, id);
create index if not exists idx_signature_field_recipient
    on signature_field(recipient_id, page_number, id);

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

create table if not exists signature_recipient_access (
    id uuid primary key default gen_random_uuid(),
    recipient_id uuid not null unique
        references signature_envelope_recipient(id) on delete cascade,
    token_version integer not null default 1 check (token_version > 0),
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

create table if not exists signature_recipient_consent (
    recipient_id uuid primary key
        references signature_envelope_recipient(id) on delete cascade,
    consent_version text not null check (btrim(consent_version) <> ''),
    consent_text text not null check (btrim(consent_text) <> ''),
    consent_text_sha256 text not null
        check (consent_text_sha256 ~ '^[0-9a-f]{64}$'),
    accepted_at timestamptz not null,
    ip_address inet,
    user_agent text
);

create table if not exists signature_evidence_event (
    id uuid primary key default gen_random_uuid(),
    signature_request_id uuid not null
        references signature_request(id) on delete cascade,
    recipient_id uuid,
    event_type text not null check (btrim(event_type) <> ''),
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

create table if not exists email_message (
    id uuid primary key default gen_random_uuid(),
    message_kind text not null
        check (message_kind in (
            'signature_invitation','signature_reminder',
            'signature_completed','signature_declined'
        )),
    recipient_email text not null check (btrim(recipient_email) <> ''),
    template_key text not null check (btrim(template_key) <> ''),
    template_payload jsonb not null default '{}'::jsonb
        check (jsonb_typeof(template_payload) = 'object'),
    dedupe_key text not null unique check (btrim(dedupe_key) <> ''),
    status text not null default 'queued'
        check (status in ('queued','sending','sent','failed','dead','cancelled')),
    provider_message_id text,
    attempt_count integer not null default 0 check (attempt_count >= 0),
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
for each row execute function docsign_touch_updated_at();

drop trigger if exists trg_signature_recipient_state_touch_updated_at
    on signature_recipient_state;
create trigger trg_signature_recipient_state_touch_updated_at
before update on signature_recipient_state
for each row execute function docsign_touch_updated_at();

drop trigger if exists trg_email_message_touch_updated_at
    on email_message;
create trigger trg_email_message_touch_updated_at
before update on email_message
for each row execute function docsign_touch_updated_at();

commit;
