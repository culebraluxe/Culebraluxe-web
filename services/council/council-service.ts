import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import { panelFor } from './panels'
import { inspectVendor } from './protocol'
import { decideRefuse } from './refuse'
import type { CouncilRepository } from './repository'
import type { CouncilModelPort } from './model-port'
import { NullModelPort } from './model-port'
import {
  COUNCIL_ADAPTER_VERSION,
  COUNCIL_OPERATIONS,
  type CouncilOperationMap,
  type CouncilSessionDto,
} from './types'

export class CouncilService extends BaseService<CouncilOperationMap> {
  readonly domain = 'council'
  readonly version = '1'
  readonly description =
    'Owns structured multi-seat decisions for app and Forge. One verdict writer. No lane.'
  protected readonly operations: ServiceOperationDefinitions<CouncilOperationMap>

  constructor(
    private readonly repository: CouncilRepository,
    infrastructure: ServiceInfrastructure = {},
    private readonly models: CouncilModelPort = new NullModelPort(),
  ) {
    super(infrastructure)

    this.operations = {
      [COUNCIL_OPERATIONS.VERIFY_VENDOR]: {
        kind: 'query',
        description: 'Compare vendor/council-pin.json to the submodule checkout.',
        authorization: 'council.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async () => this.repository.getVendorPin(),
      },

      [COUNCIL_OPERATIONS.GET]: {
        kind: 'query',
        description: 'Return one council session and its verdict.',
        authorization: 'council.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.get(request.sessionId),
      },

      [COUNCIL_OPERATIONS.GET_BY_SUBJECT]: {
        kind: 'query',
        description: 'Return the latest session for a subject.',
        authorization: 'council.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) =>
          this.repository.getBySubject(request.subjectType, request.subjectId),
      },

      [COUNCIL_OPERATIONS.CONVENE]: {
        kind: 'command',
        description: 'Refuse or open a session for one subject. Does not invent a second inflight session.',
        authorization: 'council.write',
        execution: { mode: 'ordered', partitionBy: 'subjectId' },
        handle: async (request, context) => {
          if (!request.subjectId.trim()) this.fail('SUBJECT_ID_REQUIRED', 'subjectId is required.')
          if (!request.question.trim()) this.fail('QUESTION_REQUIRED', 'question is required.')

          const vendor = inspectVendor()
          if (vendor.drift) {
            this.fail('COUNCIL_VENDOR_DRIFT', vendor.drift)
          }

          const inflight = await this.repository.findInflight(
            request.subjectType,
            request.subjectId,
          )
          if (inflight) return inflight

          const refuse = decideRefuse(request)
          const panel = panelFor(request.subjectType, request.panelId)
          const mode = refuse.refuse ? 'refuse' : request.mode === 'quick' ? 'quick' : 'duo'

          const session = await this.repository.insertSession({
            id: crypto.randomUUID(),
            subjectType: request.subjectType,
            subjectId: request.subjectId,
            domain: request.domain,
            mode,
            question: request.question.trim(),
            panel,
            status: refuse.refuse ? 'refused' : 'queued',
            refuseReason: refuse.reason,
            vendorSha: vendor.pin.sha,
            adapterVersion: COUNCIL_ADAPTER_VERSION,
            correlationId: context.correlationId,
            actorKind: context.actor.kind,
            actorId: context.actor.id,
            verdict: null,
          })

          await this.emit(
            {
              type: refuse.refuse ? 'council.refused' : 'council.convened',
              aggregateId: session.id,
              payload: {
                subjectType: session.subjectType,
                subjectId: session.subjectId,
                mode: session.mode,
                status: session.status,
              },
            },
            context,
          )
          return session
        },
      },

      [COUNCIL_OPERATIONS.RECORD_VERDICT]: {
        kind: 'command',
        description: 'Write the one verdict for a session. Does not rewrite after checkpoint.',
        authorization: 'council.write',
        execution: { mode: 'ordered', partitionBy: 'sessionId' },
        handle: async (request, context) => {
          this.assertVerdictShape(request)
          const session = await this.requireOpenSession(request.sessionId)
          const verdict = await this.repository.insertVerdict(request)
          await this.repository.markStatus(session.id, 'verdict')
          await this.emit(
            {
              type: 'council.verdict_recorded',
              aggregateId: session.id,
              payload: { sessionId: session.id, pick: verdict.pick },
            },
            context,
          )
          return verdict
        },
      },

      [COUNCIL_OPERATIONS.CHECKPOINT]: {
        kind: 'command',
        description: 'Mark a verdict confirmed/revised/reversed/inconclusive. Does not edit pick.',
        authorization: 'council.write',
        execution: { mode: 'ordered', partitionBy: 'sessionId' },
        handle: async (request, context) => {
          const session = await this.repository.get(request.sessionId)
          if (!session) this.fail('SESSION_NOT_FOUND', `No council session ${request.sessionId}`)
          if (session.status !== 'verdict' || !session.verdict) {
            this.fail('VERDICT_REQUIRED', 'checkpoint requires an existing verdict')
          }
          if (session.verdict.outcome) {
            this.fail('OUTCOME_FROZEN', 'outcome is already set; open a new session to decide again')
          }
          const updated = await this.repository.checkpoint(request)
          await this.emit(
            {
              type: 'council.checkpointed',
              aggregateId: session.id,
              payload: { sessionId: session.id, outcome: updated.outcome },
            },
            context,
          )
          return updated
        },
      },
    }
  }

  dependencies() {
    return [] as const
  }

  invariants() {
    return [
      'Council is a domain service, not a Forge lane.',
      'One inflight session per (subject_type, subject_id).',
      'One verdict per session. Checkpoint updates outcome columns only.',
      'Vendor SHA labels the protocol; it does not gate paid work.',
      'full mode is unsupported.',
      'Last-contact, identity attribution, and MLS maps are not council subjects.',
    ] as const
  }

  /**
   * Optional helper for callers that want convene + model + record in one hop.
   * Still one writer: this method calls execute() for each step.
   */
  async runDuo(session: CouncilSessionDto, brief: string) {
    if (session.status === 'refused') return session
    const draft = await this.models.deliberate({
      session,
      panel: session.panel,
      brief,
    })
    await this.repository.markStatus(session.id, 'running')
    return this.repository.insertVerdict({ ...draft, sessionId: session.id })
  }

  private async requireOpenSession(sessionId: string): Promise<CouncilSessionDto> {
    const session = await this.repository.get(sessionId)
    if (!session) this.fail('SESSION_NOT_FOUND', `No council session ${sessionId}`)
    if (session.status === 'refused') {
      this.fail('SESSION_REFUSED', 'cannot record a verdict on a refused session')
    }
    if (session.status === 'verdict') {
      this.fail('VERDICT_EXISTS', 'session already has a verdict')
    }
    return session
  }

  private assertVerdictShape(request: {
    pick: string
    reasons: string[]
    dissent: string[]
    kill: string[]
  }) {
    if (!request.pick.trim()) this.fail('PICK_REQUIRED', 'verdict.pick is required')
    if (request.reasons.length < 1 || request.reasons.length > 3) {
      this.fail('REASONS_BOUNDS', 'verdict.reasons must have 1..3 items')
    }
    if (request.dissent.length < 1) {
      this.fail('DISSENT_REQUIRED', 'verdict.dissent must have at least one objection')
    }
    if (request.kill.length < 1) {
      this.fail('KILL_REQUIRED', 'verdict.kill must have at least one criterion')
    }
  }
}
