-- When a website lead's emails were sent: the notice to the team and the visitor's confirmation.
-- Claimed atomically before sending and released if sending fails, so each lead is emailed once.
set lock_timeout = '5s';
set statement_timeout = '30s';

alter table website_intake_submission add column if not exists notified_at timestamptz;

comment on column website_intake_submission.notified_at is
    'When the lead notice and visitor confirmation were sent. Null until claimed; reset if the send fails.';
