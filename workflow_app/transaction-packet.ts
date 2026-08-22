// ---------------------------------------------------------------------------
// Transaction packet — DOC-02 (application/domain layer, workflow_app).
//
// A transaction packet is a PURE, DERIVED projection. Given a deal's coarse
// canonical stage (deal.stage) and its deal/workflow facts, it determines
// which transaction document types are required for the deal, then compares
// that requirement set against the canonical transaction_document rows
// (DOC-01, db/transaction-document.ts) and reports, per deal, which required
// types are present / missing / unresolved.
//
// Boundaries (DOC-02):
//   - No packet table. The packet is recomputed on every read from the rule
//     catalog below plus transaction_document; there is no stored packet, so
//     there is no staleness and nothing to reconcile.
//   - No signing/signature state. Presence is type/label-based: a document in
//     any non-terminal state ('draft' ... 'signed') counts as present. The
//     packet never reads signedMediaId/signedAt.
//   - No auto-creation. This module never writes; creating a required document
//     is an explicit user/application action (later DOC-05 reconciliation).
//   - No workflow_engine involvement. The XML node id remains the workflow
//     state identity; this module only consumes deal.stage as a coarse input.
//   - No hardcoded jurisdiction: every jurisdiction knob is an input fact
//     (PacketFacts), and the rule catalog is jurisdiction-neutral.
//
// Unresolved facts: a gating fact that is null (e.g. appraisalApplicable) is
// NEVER coerced to a boolean. The gated requirement is not emitted as required;
// it is surfaced as an 'unresolved' item so the operator can resolve the fact.
// The packet never invents a required document for an unresolved fact.
//
// Rejected designs (architect brief): a packet table duplicating
// transaction_document; embedding signing/signature state; inventing documents
// for unresolved facts; hardcoding a single jurisdiction.
//
// Mapping sketch (stage → required types):
//   - agreement (P&S/listing) at offer → under_contract
//   - disclosure + title for title work (under_contract)
//   - financing gated by financingApplicable; inspection/appraisal gated by
//     their applicability; tax/CRIM clearance gated by requiresCrimClearance
//   - closing documents + closing statement at closing
//   - recording/registry follow-up gated by requiresRegistryFollowup
//   - HOA/survey gated by their flags
// Long-tail requirements (CRIM clearance, HOA clearance, survey, closing
// statement, registry follow-up) reuse transaction_document's SME label:
// they are created with documentType = 'other'/'closing' plus the curated
// documentTypeLabel, and presence is matched on the exact label.
// ---------------------------------------------------------------------------

import type { TransactionDocument, TransactionDocumentType } from '../db/transaction-document'
import { listTransactionDocumentsByDeal } from '../db/transaction-document'
import type { QueryExecutor } from '../db/query-executor'

/**
 * The packet-relevant deal/workflow facts (a bounded subset of
 * DealWorkflowFacts — structurally compatible, so a getDealWorkflowFacts
 * result can be passed directly).
 *
 * Gating facts read by the DOC-02 mapping:
 *   financingApplicable, appraisalApplicable (nullable — unresolved is
 *   surfaced, never coerced) and the jurisdiction config booleans
 *   requiresCrimClearance, requiresRegistryFollowup, inspectionApplicable,
 *   requiresHoaClearance, requiresSurvey.
 *
 * closingDateScheduled, requiresNotario, requiresTitleCompany and
 * insuranceApplicable are exposed for the packet surface but are not consumed
 * by the DOC-02 mapping sketch (reserved for later DOC stories; the packet
 * never invents requirements beyond the sketch).
 */
export type PacketFacts = {
  financingApplicable: boolean | null
  closingDateScheduled: boolean
  appraisalApplicable: boolean | null
  requiresNotario: boolean
  requiresTitleCompany: boolean
  requiresCrimClearance: boolean
  requiresRegistryFollowup: boolean
  inspectionApplicable: boolean
  insuranceApplicable: boolean
  requiresSurvey: boolean
  requiresHoaClearance: boolean
}

/** One required document type in the packet (the story's "document_type + document_type_label" set element). */
export type RequiredTransactionDocumentType = {
  /** Stable packet identity (also the item key in the completeness projection). */
  key: string
  /** Human label for operator display. */
  displayLabel: string
  /** Canonical transaction_document.document_type — creatable via createTransactionDocument. */
  documentType: TransactionDocumentType
  /**
   * Curated SME label for long-tail requirements (CRIM clearance, survey, ...).
   * Null for plain canonical types. Creation and presence matching use the
   * exact label.
   */
  documentTypeLabel: string | null
}

/** A gating fact that could not be resolved (null) at a stage where the gated requirement would otherwise apply. */
export type PacketUnresolvedGate = {
  key: string
  /** The unresolved fact (e.g. 'appraisalApplicable'). */
  fact: keyof PacketFacts
  displayLabel: string
  /** What the requirement would be once the fact resolves. Informational only — never emitted as required. */
  documentType: TransactionDocumentType
  documentTypeLabel: string | null
  /** Operator-facing explanation. */
  message: string
}

type PacketRule = {
  key: string
  displayLabel: string
  documentType: TransactionDocumentType
  documentTypeLabel: string | null
  /** Stages where the requirement applies (deal.stage coarse stages). */
  stages: readonly string[]
  /**
   * Optional gating fact. When set, the requirement applies only when the
   * fact is true; when the fact is null the requirement is unresolved.
   */
  gate?: keyof PacketFacts
}

/**
 * The deal-independent required-document catalog (stage + condition →
 * document_type). This is the single source of truth for the packet; it is
 * kept in code (the story's "start without" the editable catalog table).
 */
const PACKET_RULES: readonly PacketRule[] = [
  {
    key: 'agreement',
    displayLabel: 'Purchase & Sale / Listing Agreement',
    documentType: 'agreement',
    documentTypeLabel: null,
    stages: ['offer', 'under_contract'],
  },
  {
    key: 'disclosure',
    displayLabel: 'Property Disclosure',
    documentType: 'disclosure',
    documentTypeLabel: null,
    stages: ['under_contract'],
  },
  {
    key: 'title',
    displayLabel: 'Title Evidence / Title Work',
    documentType: 'title',
    documentTypeLabel: null,
    stages: ['under_contract'],
  },
  {
    key: 'financing',
    displayLabel: 'Financing Documents',
    documentType: 'financing',
    documentTypeLabel: null,
    stages: ['under_contract'],
    gate: 'financingApplicable',
  },
  {
    key: 'inspection',
    displayLabel: 'Inspection Report',
    documentType: 'inspection',
    documentTypeLabel: null,
    stages: ['under_contract'],
    gate: 'inspectionApplicable',
  },
  {
    key: 'appraisal',
    displayLabel: 'Appraisal',
    documentType: 'appraisal',
    documentTypeLabel: null,
    stages: ['under_contract'],
    gate: 'appraisalApplicable',
  },
  {
    key: 'tax_crim_clearance',
    displayLabel: 'CRIM / Tax Clearance',
    documentType: 'other',
    documentTypeLabel: 'CRIM / tax clearance',
    stages: ['under_contract'],
    gate: 'requiresCrimClearance',
  },
  {
    key: 'hoa_clearance',
    displayLabel: 'HOA / Condo Clearance',
    documentType: 'other',
    documentTypeLabel: 'HOA / condo clearance',
    stages: ['under_contract'],
    gate: 'requiresHoaClearance',
  },
  {
    key: 'survey',
    displayLabel: 'Survey',
    documentType: 'other',
    documentTypeLabel: 'Survey',
    stages: ['under_contract'],
    gate: 'requiresSurvey',
  },
  {
    key: 'closing_documents',
    displayLabel: 'Closing Documents',
    documentType: 'closing',
    documentTypeLabel: null,
    stages: ['closed'],
  },
  {
    key: 'closing_statement',
    displayLabel: 'Closing Statement',
    documentType: 'closing',
    documentTypeLabel: 'Closing statement',
    stages: ['closed'],
  },
  {
    key: 'registry_followup',
    displayLabel: 'Registry / Recording Follow-Up',
    documentType: 'other',
    documentTypeLabel: 'Registry / recording follow-up',
    stages: ['closed'],
    gate: 'requiresRegistryFollowup',
  },
]

function rulesForStage(dealStage: string): readonly PacketRule[] {
  return PACKET_RULES.filter((rule) => rule.stages.includes(dealStage))
}

function toRequired(rule: PacketRule): RequiredTransactionDocumentType {
  return {
    key: rule.key,
    displayLabel: rule.displayLabel,
    documentType: rule.documentType,
    documentTypeLabel: rule.documentTypeLabel,
  }
}

/**
 * Pure: the deterministic set of required transaction document types for a
 * deal from its stage + facts. Facts gating a requirement (financing,
 * inspection, appraisal, CRIM, HOA, survey, registry) are applied strictly:
 * true → required, false → not required, null (unresolved) → NOT emitted here
 * (see unresolvedPacketGates). Stages outside the catalog (new_lead/qualified/
 * showing, or unknown) deterministically require nothing.
 */
export function requiredTransactionDocumentTypes(
  dealStage: string,
  facts: PacketFacts,
): RequiredTransactionDocumentType[] {
  return rulesForStage(dealStage)
    .filter((rule) => rule.gate === undefined || facts[rule.gate] === true)
    .map(toRequired)
}

/**
 * Pure: which gating facts are unresolved (null) at this stage, surfaced for
 * the operator instead of blocking the packet. Never a fabricated required
 * document — each entry names the fact that must be resolved.
 */
export function unresolvedPacketGates(
  dealStage: string,
  facts: PacketFacts,
): PacketUnresolvedGate[] {
  return rulesForStage(dealStage)
    .filter((rule) => rule.gate !== undefined && facts[rule.gate] === null)
    .map((rule) => ({
      key: rule.key,
      fact: rule.gate as keyof PacketFacts,
      displayLabel: rule.displayLabel,
      documentType: rule.documentType,
      documentTypeLabel: rule.documentTypeLabel,
      message:
        `${rule.displayLabel} cannot be determined because the fact ` +
        `'${String(rule.gate)}' is unresolved (null). Resolve the fact; no ` +
        `document is assumed.`,
    }))
}

export type TransactionPacketItemStatus = 'present' | 'missing' | 'unresolved'

export type TransactionPacketItem = RequiredTransactionDocumentType & {
  status: TransactionPacketItemStatus
  /** Ids of non-terminal transaction_document rows satisfying this item (present only). */
  documentIds: string[]
}

export type TransactionPacket = {
  dealId: string
  stage: string
  items: TransactionPacketItem[]
  presentCount: number
  missingCount: number
  unresolvedCount: number
  /** True when every required item is present and no gate is unresolved. */
  complete: boolean
}

/**
 * A document counts as "present" for a required type when it is a
 * non-terminal (not voided/superseded) transaction_document row whose
 * document_type matches, and — for long-tail items — whose
 * document_type_label equals the curated label exactly. Signature state is
 * deliberately irrelevant: presence is about having the document, not signing
 * it (DOC-02 rejects embedding signing state).
 */
function matchesRequiredItem(doc: TransactionDocument, item: RequiredTransactionDocumentType): boolean {
  if (doc.state === 'voided' || doc.state === 'superseded') return false
  if (doc.documentType !== item.documentType) return false
  if (item.documentTypeLabel === null) return true
  return doc.documentTypeLabel === item.documentTypeLabel
}

/**
 * Pure completeness projection: required types (present/missing) plus
 * unresolved gates, per deal, from the deal's transaction_document rows.
 * Never creates documents; never mutates the input rows.
 */
export function buildTransactionPacket(
  dealId: string,
  dealStage: string,
  facts: PacketFacts,
  documents: readonly TransactionDocument[],
): TransactionPacket {
  const required = requiredTransactionDocumentTypes(dealStage, facts)
  const unresolved = unresolvedPacketGates(dealStage, facts)
  const requiredByKey = new Map(required.map((item) => [item.key, item]))
  const unresolvedByKey = new Map(unresolved.map((gate) => [gate.key, gate]))

  const items: TransactionPacketItem[] = []
  for (const rule of rulesForStage(dealStage)) {
    const unresolvedGate = unresolvedByKey.get(rule.key)
    if (unresolvedGate) {
      items.push({
        key: rule.key,
        displayLabel: rule.displayLabel,
        documentType: rule.documentType,
        documentTypeLabel: rule.documentTypeLabel,
        status: 'unresolved',
        documentIds: [],
      })
      continue
    }
    const req = requiredByKey.get(rule.key)
    if (!req) continue // gate false (or otherwise not required)
    const matched = documents.filter((doc) => matchesRequiredItem(doc, req))
    items.push({
      ...req,
      status: matched.length > 0 ? 'present' : 'missing',
      documentIds: matched.map((doc) => doc.id),
    })
  }

  const presentCount = items.filter((item) => item.status === 'present').length
  const missingCount = items.filter((item) => item.status === 'missing').length
  const unresolvedCount = items.filter((item) => item.status === 'unresolved').length

  return {
    dealId,
    stage: dealStage,
    items,
    presentCount,
    missingCount,
    unresolvedCount,
    complete: presentCount === items.length && unresolvedCount === 0,
  }
}

/**
 * Read seam: load a deal's canonical transaction_document rows and project the
 * packet. Read-only — never creates documents. Facts (and stage) come from
 * getDealWorkflowFacts by the caller; `execute` is injectable for tests.
 */
export async function getTransactionPacketForDeal(
  dealId: string,
  dealStage: string,
  facts: PacketFacts,
  execute?: QueryExecutor,
): Promise<TransactionPacket> {
  const documents = await listTransactionDocumentsByDeal(dealId, execute)
  return buildTransactionPacket(dealId, dealStage, facts, documents)
}
