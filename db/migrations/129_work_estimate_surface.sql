-- V2 ripwire surface columns on work_estimate forecasts.
alter table work_estimate
  add column if not exists surface_score numeric,
  add column if not exists surface_files integer,
  add column if not exists surface_ccx integer;
