import type {
  ServiceEnvelopeFor,
  ServiceOperationMap,
} from '../core'

export const COUNCIL_ADAPTER_VERSION = '1.0.0'

export const COUNCIL_OPERATIONS = {
  CONVENE: 'convene',
  RECORD_VERDICT: 'recordVerdict',
  CHECKPOINT: 'checkpoint',
  GET: 'get',
  GET_BY_SUBJECT: 'getBySubject',
  VERIFY_VENDOR: 'verifyVendor',
} as const

export type CouncilSubjectType =
  | 'deal'
  | 'offer'
  | 'match'
  | 'listing_intake'
  | 'forge_story'
  | 'forge_architect'
  | 'forge_lead_pre'
  | 'paper_intake'

export type CouncilDomain = 'app' | 'forge'
export type CouncilMode = 'refuse' | 'duo' | 'quick'
export type CouncilSessionStatus =
  | 'queued'
  | 'running'
  | 'verdict'
  | 'refused'
  | 'failed'
export type CouncilEvidenceKind = 'FACT' | 'INFERENCE' | 'ASSUMPTION' | 'UNKNOWN'
export type CouncilOutcome = 'confirmed' | 'revised' | 'reversed' | 'inconclusive'

export type CouncilSeat = {
  id: string
  method: string
  role: string
}

export type CouncilPanel = {
  id: string
  seats: CouncilSeat[]
}

export type CouncilEvidence = {
  claim: string
  kind: CouncilEvidenceKind
}

export type CouncilNext = {
  action: string
  owner: string
  reviewAt: string | null
}

export type CouncilVerdictDto = {
  id: string
  sessionId: string
  pick: string
  reasons: string[]
  dissent: string[]
  kill: string[]
  next: CouncilNext
  evidence: CouncilEvidence[]
  outcome: CouncilOutcome | null
  outcomeAt: string | null
  outcomeNote: string | null
  createdAt: string
}

export type CouncilSessionDto = {
  id: string
  subjectType: CouncilSubjectType
  subjectId: string
  domain: CouncilDomain
  mode: CouncilMode
  question: string
  panel: CouncilPanel
  status: CouncilSessionStatus
  refuseReason: string | null
  vendorSha: string
  adapterVersion: string
  correlationId: string
  actorKind: 'user' | 'system' | 'agent'
  actorId: string | null
  createdAt: string
  updatedAt: string
  verdict: CouncilVerdictDto | null
}

export type ConveneRequest = {
  subjectType: CouncilSubjectType
  subjectId: string
  domain: CouncilDomain
  question: string
  /** Caller hint. Service may downgrade to refuse. `full` is rejected. */
  mode?: CouncilMode | 'full'
  panelId?: string
}

export type RecordVerdictRequest = {
  sessionId: string
  pick: string
  reasons: string[]
  dissent: string[]
  kill: string[]
  next: CouncilNext
  evidence: CouncilEvidence[]
  rawModel?: Readonly<Record<string, unknown>> | null
}

export type CheckpointRequest = {
  sessionId: string
  outcome: CouncilOutcome
  note?: string
}

export type GetCouncilRequest = { sessionId: string }
export type GetBySubjectRequest = {
  subjectType: CouncilSubjectType
  subjectId: string
}

export type VendorStatus = {
  ok: boolean
  pinnedSha: string
  checkoutSha: string | null
  tag: string | null
  adapterVersion: string
  drift: string | null
}

export type CouncilOperationMap = ServiceOperationMap & {
  convene: { request: ConveneRequest; response: CouncilSessionDto }
  recordVerdict: { request: RecordVerdictRequest; response: CouncilVerdictDto }
  checkpoint: { request: CheckpointRequest; response: CouncilVerdictDto }
  get: { request: GetCouncilRequest; response: CouncilSessionDto | null }
  getBySubject: { request: GetBySubjectRequest; response: CouncilSessionDto | null }
  verifyVendor: { request: Record<string, never>; response: VendorStatus }
}

export type CouncilEnvelope<K extends keyof CouncilOperationMap = keyof CouncilOperationMap> =
  ServiceEnvelopeFor<CouncilOperationMap, K>
