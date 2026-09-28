-- CulebraLuxe
-- l_call.duration_seconds must accept Apple's real duration values.
-- Migration: 161_l_call_duration_precision.sql
--
-- FOUND BY A REAL RUN: apple-calls-intake failed on its first row with
--   22P02 invalid input syntax for type integer: "606.3955090045929"
-- Apple's CallHistoryDB stores duration as a floating-point number of seconds,
-- and migration 158 typed duration_seconds as integer.
--
-- The L table is SOURCE EVIDENCE. Rounding the value at landing would be
-- interpretation - exactly what the landing layer must not do - and it would
-- lose the sub-second component the source actually recorded. So the column
-- gets the source's own precision instead: double precision, which is also what
-- node-postgres returns as a JS number, so no transport-type normalisation is
-- needed above this boundary.

begin;

alter table l_call
    alter column duration_seconds type double precision using duration_seconds::double precision;

comment on column l_call.duration_seconds is
    'Call duration in seconds, at the source''s own precision (Apple stores a float). Never rounded at landing - rounding is interpretation.';

commit;
