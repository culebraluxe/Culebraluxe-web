-- 197_gate_checks_receipt.sql
--
-- WHAT EACH CHECK ACTUALLY DID (FORGE-GATE-RECEIPT-01).
--
-- WHY A COLUMN AND NOT A SECOND VERDICT. Today the evidence says "4 skipped" with no identity: a reader can
-- see that something did not run and cannot see WHICH. Astra review feature 3 asks for the receipt, and adds
-- the constraint that makes it safe — do not introduce another verdict writer. So this is a JSONB LIST of
-- {id, status, reason} records projected from checks that already happened: the frozen proofs, the
-- architecture and migration halves of the static gate, the acceptance mapping, the negative control. The
-- status vocabulary is the repository's own: passed, failed, SKIPPED, unavailable, not-configured — and a
-- check that did not run is never recorded as passed.
--
-- No data change, no table rewrite: one nullable column, safe to apply and re-apply.
set lock_timeout = '5s';
set statement_timeout = '30s';

alter table forge_workflow_evidence
  add column if not exists gate_checks jsonb;

comment on column forge_workflow_evidence.gate_checks is
  'Per-check receipt: [{id, status, reason}] where status is passed | failed | skipped | unavailable | not-configured. A projection of checks that already happened — never a verdict. A skipped or unavailable check is never passed.';
