-- 163: remove the l_applemail rows the Envelope Index reader mis-keyed.
--
-- Context: scripts/macbridge/apple-mail-envelope-sqlite.py briefly built the replay
-- identity from messages.message_id. That column is `INTEGER NOT NULL DEFAULT 0` and
-- is SHARED by the same mail sitting in different mailboxes, so it can never be an
-- RFC Message-ID. Mail's AppleEvent bridge reports the real RFC for the same message,
-- so every record the reader landed a second time could not be seen as a duplicate by
-- the on-conflict guard. The reader was fixed in commit 26ea5f5 to read
-- message_global_data.message_id_header instead.
--
-- Measured blast radius before this delete, in PROD:
--   lisa@culebraluxe.com      154 rows = 77 keyed by RFC (correct) + 77 keyed by integer (this bug)
--   culebraluxe@gmail.com     404 rows, all RFC - untouched
--   penfield33@gmail.com       61 rows, all RFC - untouched
--
-- Scoped four ways so it cannot reach anything else: one source account, one id shape,
-- the message-id prefix, and the numeric payload. The previous reader only ever wrote
-- "message-id:<rfc containing @>" or "mail-local:<kind>:<id>", so a bare integer after
-- the prefix can only have been produced by the bug above.
--
-- Idempotent: on a second run it matches nothing. Applied to DEV it is a no-op, where
-- these rows never existed; it is recorded in both environments for the ledger.

delete from l_applemail
where source_account = 'lisa@culebraluxe.com'
  and source_message_id ~ '^message-id:-?[0-9]+$';
