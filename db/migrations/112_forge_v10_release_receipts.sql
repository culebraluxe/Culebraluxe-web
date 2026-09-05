-- ENG-FORGE-V10 — durable exact-artifact receipts for deployment and
-- production verification. A clean agent result alone is never a receipt.

alter table forge_workflow_evidence
  add column if not exists deployment_receipt text,
  add column if not exists production_verification_receipt text;
