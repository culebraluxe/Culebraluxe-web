import type {
  CheckpointRequest,
  CouncilSessionDto,
  CouncilVerdictDto,
  RecordVerdictRequest,
  VendorStatus,
} from './types'

export type InsertSessionInput = Omit<
  CouncilSessionDto,
  'verdict' | 'createdAt' | 'updatedAt'
> & {
  createdAt?: string
}

export interface CouncilRepository {
  getVendorPin(): Promise<VendorStatus>
  recordVendorPin(pin: {
    repoUrl: string
    gitSha: string
    tag: string | null
    adapterVersion: string
  }): Promise<void>
  findInflight(subjectType: string, subjectId: string): Promise<CouncilSessionDto | null>
  get(sessionId: string): Promise<CouncilSessionDto | null>
  getBySubject(subjectType: string, subjectId: string): Promise<CouncilSessionDto | null>
  insertSession(session: InsertSessionInput): Promise<CouncilSessionDto>
  markStatus(
    sessionId: string,
    status: CouncilSessionDto['status'],
    refuseReason?: string | null,
  ): Promise<CouncilSessionDto>
  insertVerdict(input: RecordVerdictRequest): Promise<CouncilVerdictDto>
  checkpoint(input: CheckpointRequest): Promise<CouncilVerdictDto>
}
