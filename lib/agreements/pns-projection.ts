// ---------------------------------------------------------------------------
// CRM-26 — Executed P&S -> canonical Deal operational projection (pure mapper).
//
// Maps ONLY the explicitly approved structured operational terms of an executed
// PR-PNS agreement into the payloads of existing canonical Deal commands.
//
//   | P&S field            | canonical command                     | transform          |
//   |----------------------|----------------------------------------|--------------------|
//   | closingDate          | deal.set_closing_date                 | date string        |
//   | inspectionDeadline   | deal.set_inspection_deadline          | date string        |
//   | financingDeadline    | deal.set_financing_deadline           | date string        |
//   | financing            | deal.set_financing_type               | Cash/Financed      |
//   | appraisalWaived      | deal.set_appraisal_required           | Yes/No (inverse)   |
//
// Hard rules (architecture spec CRM-26):
//   - Read ONLY the immutable issued source_snapshot.fieldValues. Never derive a
//     fact from a mutable draft or the current form template.
//   - An absent legitimate optional field is SKIPPED explicitly — we never invent
//     a missing value.
//   - A PRESENT but invalid/ambiguous/unmappable value stays UNRESOLVED (typed),
//     so the consumer can fail closed and leave the fact visibly awaiting review.
//   - purchasePrice and surveyDeadline are deliberately NOT mapped: purchasePrice
//     has an open source-of-truth invariant and surveyDeadline is an open
//     architecture decision (neither was approved for automatic promotion).
//   - Deterministic command IDs: `sourceId:field` (source execution event/message
//     id + the projected P&S field name) so an identical event replay is harmless.
//
// This module is pure: it imports only the canonical command-type constants and
// performs no I/O, so it is trivially unit-testable without a database.
// ---------------------------------------------------------------------------

import {
  DEAL_SET_APPRAISAL_REQUIRED,
  DEAL_SET_CLOSING_DATE,
  DEAL_SET_FINANCING_DEADLINE,
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_INSPECTION_DEADLINE,
} from '../commands/command-types'

export type PnsOperationalField =
  | 'closingDate'
  | 'inspectionDeadline'
  | 'financingDeadline'
  | 'financing'
  | 'appraisalWaived'

/** A fully-mapped operational projection destined for one canonical Deal command. */
export type DealCommandProjection = {
  /** The P&S structured field that produced this projection. */
  field: PnsOperationalField
  /** The stable canonical command type to execute. */
  commandType: string
  /** Deterministic command id = `${sourceId}:${field}` (replay-safe idempotency). */
  commandId: string
  /** The canonical deal this fact belongs to. */
  aggregateId: string
  /** The command envelope input payload (already transformed). */
  input: Record<string, unknown>
}

export type UnresolvedReason = 'invalid_date' | 'unknown_option' | 'ambiguous'

/** A present-but-invalid/unmappable field that must remain visibly unresolved. */
export type UnresolvedPnsField = {
  field: string
  reason: UnresolvedReason
  raw: unknown
}

export type PnsProjectionOutcome = {
  projections: DealCommandProjection[]
  unresolved: UnresolvedPnsField[]
  /** Fields present in the snapshot but intentionally skipped (e.g. price/survey). */
  skipped: string[]
}

export type ProjectPnsOperationalFieldsInput = {
  /** The canonical deal the agreement projects onto. */
  dealId: string
  /** Deterministic source identity (the AGREEMENT_FULLY_EXECUTED outbox message id). */
  sourceId: string
  /** The immutable issued `source_snapshot.fieldValues` (Record<string,string>). */
  fieldValues: Record<string, unknown> | null | undefined
}

function isDateString(value: unknown): value is string {
  if (typeof value !== 'string' || value.trim() === '') return false
  return !Number.isNaN(new Date(value).getTime())
}

/**
 * Map the immutable issued PR-PNS structured field values to canonical Deal
 * command projections. Pure and deterministic.
 */
export function projectPnsOperationalFields(
  input: ProjectPnsOperationalFieldsInput,
): PnsProjectionOutcome {
  const projections: DealCommandProjection[] = []
  const unresolved: UnresolvedPnsField[] = []
  const skipped: string[] = []

  const values = input.fieldValues ?? {}

  for (const def of DATE_FIELDS) {
    const raw = values[def.field]
    if (raw === undefined || raw === null || raw === '') {
      // Absent legitimate optional field: skip explicitly, never invent a value.
      continue
    }
    if (!isDateString(raw)) {
      unresolved.push({ field: def.field, reason: 'invalid_date', raw })
      continue
    }
    projections.push(
      buildProjection(def.field, def.commandType, input.dealId, input.sourceId, {
        [def.inputKey]: raw,
      }),
    )
  }

  // financing: select "Cash" | "Financed" -> cash | financed.
  const financingRaw = values['financing']
  if (financingRaw !== undefined && financingRaw !== null && financingRaw !== '') {
    const v = String(financingRaw).trim()
    if (v === 'Cash') {
      projections.push(
        buildProjection('financing', DEAL_SET_FINANCING_TYPE, input.dealId, input.sourceId, {
          financingType: 'cash',
        }),
      )
    } else if (v === 'Financed') {
      projections.push(
        buildProjection('financing', DEAL_SET_FINANCING_TYPE, input.dealId, input.sourceId, {
          financingType: 'financed',
        }),
      )
    } else {
      unresolved.push({ field: 'financing', reason: 'unknown_option', raw: financingRaw })
    }
  }

  // appraisalWaived: select "Yes" | "No" -> appraisal_required (inverse).
  const appraisalRaw = values['appraisalWaived']
  if (appraisalRaw !== undefined && appraisalRaw !== null && appraisalRaw !== '') {
    const v = String(appraisalRaw).trim()
    if (v === 'Yes') {
      // Appraisal waived -> appraisal NOT required.
      projections.push(
        buildProjection('appraisalWaived', DEAL_SET_APPRAISAL_REQUIRED, input.dealId, input.sourceId, {
          appraisalRequired: false,
        }),
      )
    } else if (v === 'No') {
      // Appraisal not waived -> appraisal required.
      projections.push(
        buildProjection('appraisalWaived', DEAL_SET_APPRAISAL_REQUIRED, input.dealId, input.sourceId, {
          appraisalRequired: true,
        }),
      )
    } else {
      unresolved.push({ field: 'appraisalWaived', reason: 'unknown_option', raw: appraisalRaw })
    }
  }

  // Fields present but intentionally not promoted (documented, never silent).
  for (const f of ['purchasePrice', 'surveyDeadline', 'deposit']) {
    if (values[f] !== undefined && values[f] !== null && values[f] !== '') {
      skipped.push(f)
    }
  }

  return { projections, unresolved, skipped }
}

function buildProjection(
  field: PnsOperationalField,
  commandType: string,
  dealId: string,
  sourceId: string,
  input: Record<string, unknown>,
): DealCommandProjection {
  return {
    field,
    commandType,
    commandId: `${sourceId}:${field}`,
    aggregateId: dealId,
    input,
  }
}

const DATE_FIELDS: ReadonlyArray<{
  field: 'closingDate' | 'inspectionDeadline' | 'financingDeadline'
  commandType: string
  inputKey: string
}> = [
  { field: 'closingDate', commandType: DEAL_SET_CLOSING_DATE, inputKey: 'closingDate' },
  {
    field: 'inspectionDeadline',
    commandType: DEAL_SET_INSPECTION_DEADLINE,
    inputKey: 'inspectionDeadline',
  },
  {
    field: 'financingDeadline',
    commandType: DEAL_SET_FINANCING_DEADLINE,
    inputKey: 'financingDeadline',
  },
]

// (implementation continues below)
