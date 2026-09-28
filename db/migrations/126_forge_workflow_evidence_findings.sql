-- ENG-FORGE-SHAPE-01 — durable per-execution Architect findings snapshot.
-- Architect completes and merges its structured findings here; Lead completion
-- reads them to enforce the shaping gate (refuse single-Smith over independent
-- required seams) against REAL Architect output before Smith launch.
alter table forge_workflow_evidence
  add column if not exists findings jsonb;
