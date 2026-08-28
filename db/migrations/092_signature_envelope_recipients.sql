-- CulebraLuxe Portal
-- One-envelope / many-recipient signature execution + audit artifact lineage.
-- Migration: 092_signature_envelope_recipients.sql

begin;

create table if not exists signature_envelope_recipient (
    id uuid primary key default gen_random_uuid(),
    signature_request_id uuid not null
        references signature_request(id)
        on delete cascade,
    execution_role text,
    execution_slot_id text,
    recipient_name text not null,
    recipient_email text not null,
    signer_order integer not null check (signer_order > 0),
    created_at timestamptz not null default now(),

    constraint signature_envelope_recipient_name_not_blank
        check (btrim(recipient_name) <> ''),
    constraint signature_envelope_recipient_email_not_blank
        check (btrim(recipient_email) <> ''),
    constraint signature_envelope_recipient_order_unique
        unique (signature_request_id, signer_order)
);

create unique index if not exists uq_signature_envelope_recipient_slot
    on signature_envelope_recipient(signature_request_id, execution_slot_id)
    where execution_slot_id is not null;

create index if not exists idx_signature_envelope_recipient_request
    on signature_envelope_recipient(signature_request_id, signer_order);

create index if not exists idx_signature_envelope_recipient_slot
    on signature_envelope_recipient(execution_slot_id)
    where execution_slot_id is not null;

alter table transaction_document
    add column if not exists signed_audit_media_id uuid
        references media(id)
        on delete restrict;

create index if not exists idx_transaction_document_signed_audit_media
    on transaction_document(signed_audit_media_id)
    where signed_audit_media_id is not null;

commit;
