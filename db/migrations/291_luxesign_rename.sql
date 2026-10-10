-- The native signing feature is Luxesign. It was built under another working name, so its tables, the permission
-- codes, the subscriptions, the outbox events and the queued email keys still carry that name. This renames them in
-- place: the rows are kept, and the constraints and foreign keys follow the tables automatically.
--
-- Not renamed: signature_request_id (a column, also used by the BoldSign tables and the workflow trace) and the
-- signature_* words that describe a signature itself; the rows of the evidence and receipt history keep the names
-- they were written with.

alter table document_sign_request rename to luxesign_config;
alter table signature_request rename to luxesign_request;
alter table signature_recipient_access rename to luxesign_recipient_access;
alter table signature_recipient_consent rename to luxesign_recipient_consent;
alter table signature_recipient_state rename to luxesign_recipient_state;
alter table signature_envelope_recipient rename to luxesign_envelope_recipient;
alter table signature_evidence_event rename to luxesign_evidence_event;
alter table signature_field rename to luxesign_field;
alter table signature_field_response rename to luxesign_field_response;

alter function docsign_touch_updated_at() rename to luxesign_touch_updated_at;
alter trigger trg_document_sign_request_touch_updated_at on luxesign_config
    rename to trg_luxesign_config_touch_updated_at;
alter trigger trg_signature_recipient_state_touch_updated_at on luxesign_recipient_state
    rename to trg_luxesign_recipient_state_touch_updated_at;

-- Permission codes: the operations the desk and the edge are granted.
update entitlement set code = regexp_replace(code, '^documentSign\.', 'luxesign.')
 where code like 'documentSign.%';

-- Subscriptions and their deliveries. The delivery rows point at the subscription by id, so that key is re-pointed
-- in the same transaction.
alter table mq_delivery drop constraint mq_delivery_subscription_id_fkey;
update mq_subscription
   set id = regexp_replace(id, '^document-sign\.', 'luxesign.'),
       routing_key = replace(routing_key, 'DOCUMENT_SIGN_', 'LUXESIGN_')
 where id like 'document-sign.%' or routing_key like '%DOCUMENT_SIGN_%';
update mq_delivery
   set subscription_id = regexp_replace(subscription_id, '^document-sign\.', 'luxesign.')
 where subscription_id like 'document-sign.%';
alter table mq_delivery add constraint mq_delivery_subscription_id_fkey
    foreign key (subscription_id) references mq_subscription(id) on delete cascade;

-- Events not yet delivered keep their meaning under the new names.
update outbox_message
   set event_type = replace(event_type, 'DOCUMENT_SIGN_', 'LUXESIGN_')
 where event_type like 'DOCUMENT_SIGN_%';

-- Emails already queued or sent keep their template under the new key.
update email_message
   set template_key = regexp_replace(template_key, '^document-sign\.', 'luxesign.')
 where template_key like 'document-sign.%';
