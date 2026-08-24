import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  projectPnsOperationalFields,
  type PnsProjectionOutcome,
} from '../../lib/agreements/pns-projection'
import {
  DEAL_SET_APPRAISAL_REQUIRED,
  DEAL_SET_CLOSING_DATE,
  DEAL_SET_FINANCING_DEADLINE,
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_INSPECTION_DEADLINE,
} from '../../lib/commands/command-types'

// ---------------------------------------------------------------------------
// CRM-26 — pure PR-PNS -> canonical Deal operational mapper proofs.
// No database, no packages: the mapper is a pure function over the immutable
// issued source_snapshot.fieldValues.
// ---------------------------------------------------------------------------

const SOURCE_ID = 'evt-111'
const DEAL_ID = 'deal-9'

/** A realistic immutable PR-PNS issued snapshot.fieldValues. */
function realFieldValues(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    buyerName: 'Buyer One',
    sellerName: 'Seller',
    property: '123 Main St',
    purchasePrice: '450000',
    deposit: '15000',
    closingDate: '2026-08-31',
    financing: 'Financed',
    financingDeadline: '2026-08-15',
    appraisalWaived: 'No',
    surveyDeadline: '2026-08-20',
    inspectionDeadline: '2026-08-10',
    ...overrides,
  }
}

function project(overrides: Record<string, unknown> = {}): PnsProjectionOutcome {
  return projectPnsOperationalFields({
    dealId: DEAL_ID,
    sourceId: SOURCE_ID,
    fieldValues: realFieldValues(overrides),
  })
}

function byField(outcome: PnsProjectionOutcome, field: string) {
  return outcome.projections.find((p) => p.field === field)
}

test('CRM-26 test 1: a real immutable PR-PNS snapshot maps all approved operational terms', () => {
  const outcome = project()

  // Closing date.
  const closing = byField(outcome, 'closingDate')
  assert.ok(closing, 'closingDate must project')
  assert.equal(closing.commandType, DEAL_SET_CLOSING_DATE)
  assert.deepEqual(closing.input, { closingDate: '2026-08-31' })
  assert.equal(closing.aggregateId, DEAL_ID)
  assert.equal(closing.commandId, `${SOURCE_ID}:closingDate`)

  // Inspection deadline.
  const inspection = byField(outcome, 'inspectionDeadline')
  assert.ok(inspection, 'inspectionDeadline must project')
  assert.equal(inspection.commandType, DEAL_SET_INSPECTION_DEADLINE)
  assert.deepEqual(inspection.input, { inspectionDeadline: '2026-08-10' })

  // Financing deadline.
  const financingDeadline = byField(outcome, 'financingDeadline')
  assert.ok(financingDeadline, 'financingDeadline must project')
  assert.equal(financingDeadline.commandType, DEAL_SET_FINANCING_DEADLINE)
  assert.deepEqual(financingDeadline.input, { financingDeadline: '2026-08-15' })

  // Financing "Financed" -> financed.
  const financing = byField(outcome, 'financing')
  assert.ok(financing, 'financing must project')
  assert.equal(financing.commandType, DEAL_SET_FINANCING_TYPE)
  assert.deepEqual(financing.input, { financingType: 'financed' })

  // Appraisal "No" -> appraisal REQUIRED (inverse mapping).
  const appraisal = byField(outcome, 'appraisalWaived')
  assert.ok(appraisal, 'appraisalWaived must project')
  assert.equal(appraisal.commandType, DEAL_SET_APPRAISAL_REQUIRED)
  assert.deepEqual(appraisal.input, { appraisalRequired: true })

  // Cash + waived -> financed:false and appraisal not required.
  const cash = project({ financing: 'Cash', appraisalWaived: 'Yes' })
  assert.equal(byField(cash, 'financing')?.input.financingType, 'cash')
  assert.equal(byField(cash, 'appraisalWaived')?.input.appraisalRequired, false)

  // purchasePrice / surveyDeadline / deposit present but intentionally NOT promoted.
  assert.ok(outcome.skipped.includes('purchasePrice'), 'price must be documented as skipped')
  assert.ok(outcome.skipped.includes('surveyDeadline'), 'survey must be documented as skipped')
  assert.ok(outcome.skipped.includes('deposit'), 'deposit must be documented as skipped')
  assert.deepEqual(outcome.unresolved, [])
})

test('CRM-26 test 2: missing optional fields do not invent values', () => {
  const outcome = projectPnsOperationalFields({
    dealId: DEAL_ID,
    sourceId: SOURCE_ID,
    fieldValues: {},
  })
  assert.equal(outcome.projections.length, 0, 'no projection for an empty snapshot')
  assert.deepEqual(outcome.unresolved, [], 'no unresolved for absent fields')
  assert.equal(outcome.skipped.length, 0)
})


test('CRM-26 test 3: invalid or ambiguous populated values remain visibly unresolved', () => {
  const outcome = project({
    closingDate: 'not-a-date',
    financing: 'Maybe',
    appraisalWaived: 'Perhaps',
  })

  assert.equal(byField(outcome, 'closingDate'), undefined, 'invalid date must not project')
  assert.equal(byField(outcome, 'financing'), undefined, 'unknown financing must not project')
  assert.equal(byField(outcome, 'appraisalWaived'), undefined, 'unknown appraisal must not project')

  const unresolvedByField = new Map(outcome.unresolved.map((u) => [u.field, u]))
  assert.equal(unresolvedByField.get('closingDate')?.reason, 'invalid_date')
  assert.equal(unresolvedByField.get('financing')?.reason, 'unknown_option')
  assert.equal(unresolvedByField.get('appraisalWaived')?.reason, 'unknown_option')
  assert.ok(outcome.unresolved.length >= 3, 'all three bad values must be unresolved')
})

test('CRM-26 test 3b: valid sibling fields still project alongside an unresolved field', () => {
  // Only financing is bad; the rest still project (partial projection + delivery fail).
  const outcome = project({ financing: 'Nope' })
  assert.equal(byField(outcome, 'financing'), undefined)
  assert.ok(byField(outcome, 'closingDate'))
  assert.ok(byField(outcome, 'inspectionDeadline'))
  assert.equal(outcome.unresolved.length, 1)
  assert.equal(outcome.unresolved[0].field, 'financing')
})

