// ---------------------------------------------------------------------------
// CRM-26 — Executed P&S -> canonical Deal operational projection (pure mapper).
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

export type DealCommandProjection = {
  field: PnsOperationalField
  commandType: string
  commandId: string
  aggregateId: string
  input: Record<string, unknown>
}

export type UnresolvedReason = 'invalid_date' | 'unknown_option' | 'ambiguous'

export type UnresolvedPnsField = {
  field: string
  reason: UnresolvedReason
  raw: unknown
}

export type PnsProjectionOutcome = {
  projections: DealCommandProjection[]
  unresolved: UnresolvedPnsField[]
  skipped: string[]
}

export type ProjectPnsOperationalFieldsInput = {
  dealId: string
  sourceId: string
  fieldValues: Record<string, unknown> | null | undefined
}

function isDateString(value: unknown): value is string {
  if (typeof value !== 'string' || value.trim() === '') return false
  return !Number.isNaN(new Date(value).getTime())
}

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

  const financingRaw = values['financing']
  if (financingRaw !== undefined && financingRaw !== null && financingRaw !== '') {
    const v = String(financingRaw).trim()
    if (v === 'Cash') {
      projections.push(
        buildProjection('financing', DEAL_SET_FINANCING_TYPE, input.dealId, input.sourceId, {
          financingType: 'cash',
        }),
      )
    } else if (v === 'Financed' || v === 'Bank' || v === 'Owner' || v === 'Blend') {
      projections.push(
        buildProjection('financing', DEAL_SET_FINANCING_TYPE, input.dealId, input.sourceId, {
          financingType: 'financed',
        }),
      )
    } else if (v === 'Show All') {
      skipped.push('financing')
    } else {
      unresolved.push({ field: 'financing', reason: 'unknown_option', raw: financingRaw })
    }
  }

  const appraisalRaw = values['appraisalWaived']
  if (appraisalRaw !== undefined && appraisalRaw !== null && appraisalRaw !== '') {
    const v = String(appraisalRaw).trim()
    if (v === 'Yes') {
      projections.push(
        buildProjection('appraisalWaived', DEAL_SET_APPRAISAL_REQUIRED, input.dealId, input.sourceId, {
          appraisalRequired: false,
        }),
      )
    } else if (v === 'No') {
      projections.push(
        buildProjection('appraisalWaived', DEAL_SET_APPRAISAL_REQUIRED, input.dealId, input.sourceId, {
          appraisalRequired: true,
        }),
      )
    } else {
      unresolved.push({ field: 'appraisalWaived', reason: 'unknown_option', raw: appraisalRaw })
    }
  }

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
