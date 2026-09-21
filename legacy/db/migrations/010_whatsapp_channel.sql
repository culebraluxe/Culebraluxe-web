-- CulebraLuxe Portal
-- M-1 / CRM-07: WhatsApp interaction channel (additive widening)
-- Migration: 010_whatsapp_channel.sql
--
-- Architecture note:
-- - WhatsApp is a canonical interaction *channel*, not a new identity type.
--   WhatsApp actors continue to resolve through person_identity phone
--   (strict E.164) exactly as SMS/iMessage/calls do.
-- - Source idempotency reuses the existing (source_system, source_external_id)
--   unique index; a future connector would emit
--   source.system = communications:<provider>:<accountNamespace> and
--   source.externalId = whatsapp:<providerMessageId>.
-- - No provider integration, OAuth, or credentials are added here.

begin;

alter table interaction
    drop constraint interaction_channel_check;

alter table interaction
    add constraint interaction_channel_check
    check (channel in (
        'website',
        'email',
        'call',
        'imessage',
        'sms',
        'calendar',
        'meeting',
        'showing',
        'document',
        'manual',
        'note',
        'whatsapp'
    ));

commit;
