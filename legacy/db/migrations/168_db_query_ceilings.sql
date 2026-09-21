-- CulebraLuxe Portal
-- Database query ceilings — the Oracle-like reaping we assumed Neon already did.
-- Migration: 168_db_query_ceilings.sql
--
-- Neon does NOT kill a hung query. It suspends an idle COMPUTE, but a query that
-- keeps running (or a connection parked inside an open transaction) will hold its
-- pool slot until the process dies. The app pool is `max: 5`, so five of those take
-- the whole site down — which is the cascade ENG-DB-RESILIENCE-01 exists to stop.
--
-- Set as DATABASE DEFAULTS rather than in the pool config, because Neon's POOLED
-- endpoint REJECTS startup parameters (`08P01 unsupported startup parameter in
-- options: statement_timeout`, verified 2026-09-13) — passing it there fails every
-- connection. A database default needs no startup parameter, so it survives the
-- pooler, and it is enforced by the SERVER: the query is reaped, not merely
-- abandoned by the caller.
--
--   statement_timeout                   60s — a single statement running too long
--   idle_in_transaction_session_timeout 15s — a slot parked in an open transaction
--
-- 60s is deliberately generous: a migration, a materialized-view refresh, or a
-- large backfill may legitimately run for a while, and killing those would be worse
-- than the problem. It is a runaway ceiling, not a performance target. A caller
-- that needs longer can `SET LOCAL statement_timeout` inside its own transaction.
--
-- Applied with a DO block because ALTER DATABASE cannot take a function call: the
-- database name differs between DEV and PROD and must be resolved at run time.

begin;

do $$
begin
  execute format('alter database %I set statement_timeout = %L', current_database(), '60s');
  execute format(
    'alter database %I set idle_in_transaction_session_timeout = %L',
    current_database(),
    '15s'
  );
end $$;

commit;
