# CRM26-CONTRACT-CUT-01 — P&S execution leaves Deal

## Goal

Finish the P&S strangler cut so a fully executed canonical PR-PNS document drives the Contract service and a Contract-subject residential workflow. New P&S execution must not project canonical terms or stage changes back into Deal.

## Authority / scope

The canonical path is:

`Contract-bound PR-PNS issued evidence -> AGREEMENT_FULLY_EXECUTED -> Contract.execute(evidenceDocumentId) -> Contract-subject RE_supermodel -> pns_executed`

### In scope

- Agreement completion reads explicit `transaction_document.contract_id`.
- `AGREEMENT_FULLY_EXECUTED` carries `contractId`.
- CRM26 requires Contract lineage and calls `contract.execute`.
- CRM26 starts/reuses a Contract-subject workflow and catches it up through P&S preparation/execution.
- Contract-subject `mark_under_contract` must not dual-write `deal.stage`; Contract execution is authoritative.
- Keep legacy Deal workflow/fact compatibility only for later workflow areas that are not part of this P&S execution cut.

### Out of scope

- Refactoring the Deal page or deleting Deal schema/code.
- Showing Report service.
- Offer Letter service.
- General Form-binding extraction.
- Rewriting later closing/inspection workflow commands in this story.
- PROD data cleanup or heuristic Contract backfill.

## Guardrails

- Do not infer Contract from Deal, Person, Property, template, or latest-row heuristics.
- Contract-bound PR-PNS documents may have `deal_id = null`.
- Existing real Listing Agreements remain untouched; their legacy issuance path is not changed.
- Old/fake P&S history does not need migration compatibility in CRM26.
- No PROD/Neon business-data mutation.
- No full regression.

## Acceptance criteria

1. A Contract-bound PR-PNS with `deal_id = null` can emit `AGREEMENT_FULLY_EXECUTED` with `contractId`.
2. CRM26 rejects a P&S event without explicit Contract lineage.
3. CRM26 performs no Deal command projection.
4. CRM26 calls Contract execution with the issued document as evidence before workflow advancement.
5. The workflow instance subject is `contract`, not `deal`.
6. If execution arrives before P&S workflow preparation was manually advanced, the workflow catches up `pns_preparation -> pns_executed` safely.
7. Replayed completion remains harmless.
8. Contract-subject `deal.set_stage_under_contract` is suppressed as an obsolete dual-write.
9. Legacy Listing Agreement issuance code is unchanged.

## Assay (scoped)

```sh
pnpm exec tsx --test workflow_app/tests/crm26-consumer.test.ts workflow_app/tests/agreement-execution-command.test.ts
git diff --check
```

Do not run the full suite unless explicitly authorized.
