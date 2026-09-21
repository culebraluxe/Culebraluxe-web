-- 184_l_whatsapp_context_id.sql
--
-- APP-WHATSAPP-ATTRIBUTION-01: a WhatsApp message must say WHICH LINE received
-- it, WHAT THREAD it belongs to, and the envelope facts it was read from.
--
-- WHY A COLUMN AND NOT REUSE OF conversation_id: `conversation_id` is being
-- repurposed to the STABLE thread key (receiving line + contact), so the quoted
-- reply id Meta sends in `message.context.id` needs its own home. Without this
-- column the quoted-reply linkage is silently dropped the moment
-- conversation_id changes meaning - one fact, one home.
--
-- Non-destructive: one added nullable column, no existing row or column touched.
-- Replay key (source_account, source_message_id) is unchanged.

alter table l_whatsapp
    add column if not exists context_id text;

comment on column l_whatsapp.context_id is
    'Meta message.context.id - the quoted reply this message answers, kept separate from conversation_id (the stable receiving-line + contact thread key).';

create index if not exists l_whatsapp_context_idx on l_whatsapp (context_id);
