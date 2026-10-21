-- 302 — Drop dormant BoldSign provider tables. BoldSign is retired; Luxesign is the signing product.
--
-- No Rust code reads or writes these tables after the provider removal. They are historical provider
-- state, not part of the canonical signature domain (`signature_request` stays). These are dormant retired provider tables.
BEGIN;
drop table if exists bold_sign_webhook_event;
drop table if exists bold_sign_request;
COMMIT;
