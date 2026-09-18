-- ENG-FORGE-FENCE-CAN-FAIL-01 — a fence that has never been shown to fail is not proof.
--
-- The negative-control outcome is a durable evidence fact, written by exactly one writer
-- (`mergeForgeWorkflowEvidence` in db/forge-workflow-evidence.ts). `negative_control_ran`
-- records whether the control actually executed; `negative_control_killing_assertion` names
-- the mapped assertions that went red under it (stored as a JSON array of names, because an
-- assertion name may itself contain the separator a joined list would use). A control that
-- ran and killed no assertion is UNPROVEN, never proof — QA reports that verdict and this row
-- carries the fact.
--
-- Both columns are nullable on purpose: an omitted write preserves stored truth via coalesce,
-- exactly like every other evidence column.

alter table forge_workflow_evidence
  add column if not exists negative_control_ran boolean,
  add column if not exists negative_control_killing_assertion text;
