-- 276 — Native document signing: who is copied on the outcome, and how often a waiting signer is reminded.
--
-- `copy_to_emails`: people who receive the completed document (and the decline notice) without signing — for a listing
-- contract, the broker. The operator who sent the envelope is always copied; this list is for everyone else.
--
-- `reminder_every_days`: a signer whose turn it is, who has not finished, is reminded this often (3 reminders at most,
-- counted from the durable email log, so no new state is kept here). Zero turns reminders off.
--
-- Both are read by name where used (never by `select *`), so they are safe to add ahead of the code.

begin;

alter table document_sign_request
    add column if not exists copy_to_emails text[] not null default '{}',
    add column if not exists reminder_every_days integer not null default 3
        check (reminder_every_days >= 0 and reminder_every_days <= 60);

commit;
