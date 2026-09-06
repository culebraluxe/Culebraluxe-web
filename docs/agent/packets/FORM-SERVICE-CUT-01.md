# FORM-SERVICE-CUT-01 — Showing Report + Offer Letter service cut

## Goal

Finish the form-to-services tail after PR-PNS.

- SHOW-RPT becomes an explicit Person + Property Showing report.
- OFFER-01 becomes an explicit Person + Property + Contract draft.
- Deal may be used only as a launcher to resolve explicit Person/Property context; new saved SHOW-RPT/OFFER-01 forms do not retain Deal as canonical context.
- Extract only the reusable Form/service binding mechanics. Do not create a generic business-domain abstraction that hides Showing or Contract semantics.

## In scope

- Existing `showing` aggregate/service boundary and report facts.
- Explicit `document_form_instance.showing_id` lineage.
- OFFER-01 v2 with Person/Property bindings and Contract-owned offer terms.
- Save/Issue/Send service synchronization through the existing Forms server-action boundary.
- Shared service context/result helpers and Form binding dispatcher.
- Scoped mapper/template tests.

## Out of scope

- Deal page redesign or removal.
- Rewriting the remaining legacy Deal workflow after P&S execution.
- New Offer aggregate/service/table.
- Listing Agreement changes.
- Heuristic backfill from Person, Property, Deal, or latest rows.
- Broad Forms UI redesign.

## Architecture

### Showing Report

`Form SHOW-RPT -> Person + Property -> ShowingService.saveReport -> showing`

The existing `showing` row owns showing date, duration, outcome, interest score, feedback, and follow-up notes. BUYER/PROSPECT remains contextual; it is not written as intrinsic Person truth.

### Offer Letter

`Form OFFER-01 v2 -> Person + Property -> ContractService.saveDraft(offer_letter)`

Offer economics and terms live in Contract facts. BUYER is a Contract Role mapped to the explicit Person. No Deal field receives a projection.

### Shared seam

`syncFormServiceBinding()` owns only Form-to-service orchestration. Template-specific mappers remain separate.

## Compatibility / protection

- LISTING-01 is untouched; existing real Listing Agreements remain on the protected legacy issuance path.
- OFFER-01 v1 remains registered so old saved forms can still open against their exact version.
- New OFFER-01 creation uses v2.
- Existing legacy SHOW-RPT/OFFER-01 drafts with a Deal FK may use that exact FK once to resolve direct Person/Property context; the Form is then rebound with `deal_id = null`.
- No latest-row or Person/Property heuristic is permitted.

## Acceptance criteria

1. SHOW-RPT save creates/enriches exactly one explicit Showing and stores date/outcome/interest/feedback/follow-up.
2. SHOW-RPT does not write Deal.
3. OFFER-01 v2 saves offer terms to a Contract draft and maps BUYER Person + subject Property.
4. OFFER-01 v2 does not write Deal/Offer tables.
5. Save synchronizes mutable service truth; Issue and Send synchronize before immutable evidence/provider work.
6. OFFER-01 v1 remains resolvable; v2 is active.
7. Listing Agreement code/data path is unchanged.
8. No heuristic backfill.

## Assay — SCOPED

```sh
pnpm exec tsx --test workflow_app/tests/form-service-binding.test.ts
```

Do not run the full suite unless explicitly authorized.
