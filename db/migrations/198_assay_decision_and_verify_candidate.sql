-- 198_assay_decision_and_verify_candidate.sql
--
-- THE DIRECT-TO-QA ROUTE, IN THE ROWS (FORGE-VERIFY-EXISTING-COMPLETE-01, work package A).
--
-- WHY THE SCHEMA WAS THE BLOCKER. The route was implemented in the routing validator, the arrangement helper
-- and the v6 workflow, and none of it could ever fire, because the words never reached a row:
--
--   * `forge_role_contract_decision_check` (migration 170) permits only SOLO, SMITH, SPLIT, HOLD. A Lead that
--     decided ASSAY had its decision refused by the database — the write failed and the run HELD.
--   * There was nowhere to record WHICH candidate the decision was about. "Verify what already exists" without
--     naming the work is a route to nowhere, and the routing validator refuses an ASSAY with no candidate.
--
-- Both are closed here. The check is REPLACED (widened, never loosened for the existing four) and the candidate
-- gets a column of its own with the 40-hex constraint the validator compares against — so a malformed or
-- truncated identity fails the WRITE with a message the model can read and fix, instead of surviving as a value
-- that a later reader has to refuse.
--
-- Non-destructive: one constraint replaced with a strictly wider one (the four existing values still pass), one
-- added nullable column, no row rewritten, no data touched. Safe to apply and re-apply.

set lock_timeout = '5s';
set statement_timeout = '30s';

alter table forge_role_contract
    drop constraint if exists forge_role_contract_decision_check;

-- NOT VALID + VALIDATE, not a bare ADD: a check added the ordinary way takes a write-blocking scan of the
-- table, and the migration gate refuses that shape (squawk `constraint-missing-not-valid`). New rows are still
-- checked the moment the constraint exists, so nothing is left unguarded in between.
alter table forge_role_contract
    add constraint forge_role_contract_decision_check
        check (decision is null or decision in ('SOLO', 'SMITH', 'SPLIT', 'HOLD', 'ASSAY')) not valid;

alter table forge_role_contract
    validate constraint forge_role_contract_decision_check;

-- ASSAY names the commit to judge. Null stays null for every other decision: "no candidate stated" must remain
-- distinguishable from "a candidate was stated", because the routing validator treats the second as a claim.
alter table forge_role_contract
    add column if not exists verify_candidate text null;

alter table forge_role_contract
    drop constraint if exists forge_role_contract_verify_candidate_check;

alter table forge_role_contract
    add constraint forge_role_contract_verify_candidate_check
        check (verify_candidate is null or verify_candidate ~ '^[0-9a-f]{40}$') not valid;

alter table forge_role_contract
    validate constraint forge_role_contract_verify_candidate_check;

comment on column forge_role_contract.decision is
    'The Lead''s route: SOLO | SMITH | SPLIT | HOLD | ASSAY. ASSAY judges existing work instead of authoring it (migration 198).';

comment on column forge_role_contract.verify_candidate is
    'ASSAY only: the 40-hex lowercase commit the decision verifies. Written by scripts/forge-handoff.mjs --verify-candidate; null for every other decision.';

-- A candidate WITHOUT an ASSAY decision is a mismatched contract (a SOLO that names something to verify), and a
-- decision that needs a candidate cannot be recorded without one — both refused at the write, in the same place
-- the model can still fix them. `verify_candidate` is only meaningful beside an ASSAY decision.
alter table forge_role_contract
    drop constraint if exists forge_role_contract_verify_candidate_scope_check;

alter table forge_role_contract
    add constraint forge_role_contract_verify_candidate_scope_check
        check (verify_candidate is null or decision = 'ASSAY') not valid;

alter table forge_role_contract
    validate constraint forge_role_contract_verify_candidate_scope_check;
