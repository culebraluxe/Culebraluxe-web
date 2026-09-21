-- CulebraLuxe Portal
-- Query ceilings, corrected after review against Neon's own defaults.
-- Migration: 169_db_query_ceilings_tuned.sql
--
-- 168 set 60s / 15s. Reviewing that against real intake/batch load found 15s too
-- tight: `idle_in_transaction_session_timeout` fires when a session holds a
-- transaction open while doing NOTHING — and a batch job that BEGINs, computes in
-- JS for a while, then writes is exactly that shape. Neon's own default is 5min
-- for this reason.
--
-- The ceilings now match the three zombie kinds Neon actually has:
--   idle in transaction  — Neon already kills at 5min; we tighten to 60s
--   waiting for a pool slot — PgBouncer already kills at ~120s; nothing to do
--   actually RUNNING     — NOTHING kills it. This is the one we own, and it is
--                          why statement_timeout exists.
--
-- 2min for a running statement: generous enough for a large intake batch, tight
-- enough that a runaway join cannot own a pool slot indefinitely. Anything that
-- legitimately needs longer says so explicitly, in its own transaction:
--
--   begin;
--   set local statement_timeout = '5min';
--   -- long work
--   commit;
--
-- and when something HAS escaped the fence, there is a chainsaw:
--   node --import tsx --env-file=.env.local scripts/db-zombies.mjs

begin;

do $$
begin
  execute format('alter database %I set statement_timeout = %L', current_database(), '2min');
  execute format(
    'alter database %I set idle_in_transaction_session_timeout = %L',
    current_database(),
    '60s'
  );
end $$;

commit;
