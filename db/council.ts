// CIL-02: replace this import with the live DatabaseGateway helper used by
// sibling modules under db/*. Do not add a second Neon client.
import { query } from '../neon'
import type { CouncilRepository, InsertSessionInput } from '../services/council/repository'
import type {
  CheckpointRequest,
  CouncilSessionDto,
  CouncilVerdictDto,
  RecordVerdictRequest,
  VendorStatus,
} from '../services/council/types'
import { COUNCIL_ADAPTER_VERSION } from '../services/council/types'
import { inspectVendor } from '../services/council/protocol'

/**
 * Only writer of council_session / council_verdict / council_vendor_pin.
 * Normalize driver values before they leave this file.
 */
export function createCouncilRepository(): CouncilRepository {
  return {
    async getVendorPin() {
      const inspected = inspectVendor()
      const rows = await query<{
        git_sha: string
        tag: string | null
        adapter_version: string
      }>('select git_sha, tag, adapter_version from council_vendor_pin where id = $1', [
        'singleton',
      ])
      const row = rows[0]
      return {
        ok: !inspected.drift,
        pinnedSha: inspected.pin.sha,
        checkoutSha: inspected.checkoutSha,
        tag: inspected.pin.tag,
        adapterVersion: row?.adapter_version ?? COUNCIL_ADAPTER_VERSION,
        drift: inspected.drift,
      } satisfies VendorStatus
    },

    async recordVendorPin(pin) {
      await query(
        `insert into council_vendor_pin (id, repo_url, git_sha, tag, adapter_version, recorded_at)
         values ('singleton', $1, $2, $3, $4, now())
         on conflict (id) do update set
           repo_url = excluded.repo_url,
           git_sha = excluded.git_sha,
           tag = excluded.tag,
           adapter_version = excluded.adapter_version,
           recorded_at = now()`,
        [pin.repoUrl, pin.gitSha, pin.tag, pin.adapterVersion],
      )
    },

    async findInflight(subjectType, subjectId) {
      const rows = await query<SessionRow>(
        `select * from council_session
          where subject_type = $1 and subject_id = $2
            and status in ('queued', 'running')
          limit 1`,
        [subjectType, subjectId],
      )
      if (!rows[0]) return null
      return hydrate(rows[0], null)
    },

    async get(sessionId) {
      const session = await query<SessionRow>(
        'select * from council_session where id = $1',
        [sessionId],
      )
      if (!session[0]) return null
      const verdict = await query<VerdictRow>(
        'select * from council_verdict where session_id = $1',
        [sessionId],
      )
      return hydrate(session[0], verdict[0] ?? null)
    },

    async getBySubject(subjectType, subjectId) {
      const session = await query<SessionRow>(
        `select * from council_session
          where subject_type = $1 and subject_id = $2
          order by created_at desc
          limit 1`,
        [subjectType, subjectId],
      )
      if (!session[0]) return null
      const verdict = await query<VerdictRow>(
        'select * from council_verdict where session_id = $1',
        [session[0].id],
      )
      return hydrate(session[0], verdict[0] ?? null)
    },

    async insertSession(session: InsertSessionInput) {
      const rows = await query<SessionRow>(
        `insert into council_session (
           id, subject_type, subject_id, domain, mode, question, panel,
           status, refuse_reason, vendor_sha, adapter_version,
           correlation_id, actor_kind, actor_id
         ) values (
           $1,$2,$3,$4,$5,$6,$7::jsonb,$8,$9,$10,$11,$12,$13,$14
         ) returning *`,
        [
          session.id,
          session.subjectType,
          session.subjectId,
          session.domain,
          session.mode,
          session.question,
          JSON.stringify(session.panel),
          session.status,
          session.refuseReason,
          session.vendorSha,
          session.adapterVersion,
          session.correlationId,
          session.actorKind,
          session.actorId,
        ],
      )
      return hydrate(rows[0], null)
    },

    async markStatus(sessionId, status, refuseReason = null) {
      const rows = await query<SessionRow>(
        `update council_session
            set status = $2,
                refuse_reason = coalesce($3, refuse_reason),
                updated_at = now()
          where id = $1
          returning *`,
        [sessionId, status, refuseReason ?? null],
      )
      if (!rows[0]) throw new Error(`SESSION_NOT_FOUND: ${sessionId}`)
      return hydrate(rows[0], null)
    },

    async insertVerdict(input: RecordVerdictRequest) {
      const rows = await query<VerdictRow>(
        `insert into council_verdict (
           session_id, pick, reasons, dissent, kill,
           next_action, next_owner, review_at, evidence, raw_model
         ) values (
           $1,$2,$3::jsonb,$4::jsonb,$5::jsonb,$6,$7,$8,$9::jsonb,$10::jsonb
         ) returning *`,
        [
          input.sessionId,
          input.pick,
          JSON.stringify(input.reasons),
          JSON.stringify(input.dissent),
          JSON.stringify(input.kill),
          input.next.action,
          input.next.owner,
          input.next.reviewAt,
          JSON.stringify(input.evidence),
          input.rawModel ? JSON.stringify(input.rawModel) : null,
        ],
      )
      return asVerdict(rows[0])
    },

    async checkpoint(input: CheckpointRequest) {
      const rows = await query<VerdictRow>(
        `update council_verdict
            set outcome = $2,
                outcome_at = now(),
                outcome_note = $3
          where session_id = $1
            and outcome is null
          returning *`,
        [input.sessionId, input.outcome, input.note ?? null],
      )
      if (!rows[0]) throw new Error(`CHECKPOINT_FAILED: ${input.sessionId}`)
      return asVerdict(rows[0])
    },
  }
}

type SessionRow = {
  id: string
  subject_type: CouncilSessionDto['subjectType']
  subject_id: string
  domain: CouncilSessionDto['domain']
  mode: CouncilSessionDto['mode']
  question: string
  panel: CouncilSessionDto['panel'] | string
  status: CouncilSessionDto['status']
  refuse_reason: string | null
  vendor_sha: string
  adapter_version: string
  correlation_id: string
  actor_kind: CouncilSessionDto['actorKind']
  actor_id: string | null
  created_at: Date | string
  updated_at: Date | string
}

type VerdictRow = {
  id: string
  session_id: string
  pick: string
  reasons: unknown
  dissent: unknown
  kill: unknown
  next_action: string
  next_owner: string | null
  review_at: Date | string | null
  evidence: unknown
  outcome: CouncilVerdictDto['outcome']
  outcome_at: Date | string | null
  outcome_note: string | null
  created_at: Date | string
}

function iso(value: Date | string | null): string | null {
  if (value == null) return null
  return value instanceof Date ? value.toISOString() : new Date(value).toISOString()
}

function asJson<T>(value: unknown): T {
  return (typeof value === 'string' ? JSON.parse(value) : value) as T
}

function asVerdict(row: VerdictRow): CouncilVerdictDto {
  return {
    id: row.id,
    sessionId: row.session_id,
    pick: row.pick,
    reasons: asJson<string[]>(row.reasons),
    dissent: asJson<string[]>(row.dissent),
    kill: asJson<string[]>(row.kill),
    next: {
      action: row.next_action,
      owner: row.next_owner ?? '',
      reviewAt: iso(row.review_at),
    },
    evidence: asJson<CouncilVerdictDto['evidence']>(row.evidence),
    outcome: row.outcome,
    outcomeAt: iso(row.outcome_at),
    outcomeNote: row.outcome_note,
    createdAt: iso(row.created_at) ?? '',
  }
}

function hydrate(row: SessionRow, verdict: VerdictRow | null): CouncilSessionDto {
  return {
    id: row.id,
    subjectType: row.subject_type,
    subjectId: row.subject_id,
    domain: row.domain,
    mode: row.mode,
    question: row.question,
    panel: asJson(row.panel),
    status: row.status,
    refuseReason: row.refuse_reason,
    vendorSha: row.vendor_sha,
    adapterVersion: row.adapter_version,
    correlationId: row.correlation_id,
    actorKind: row.actor_kind,
    actorId: row.actor_id,
    createdAt: iso(row.created_at) ?? '',
    updatedAt: iso(row.updated_at) ?? '',
    verdict: verdict ? asVerdict(verdict) : null,
  }
}
