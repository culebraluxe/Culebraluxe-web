import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import {
  COMMS_SOURCE_SLOT_COUNT,
  isFaceTimeInteraction,
  momentChannelFor,
  sourceChannelFor,
  sourceChannelLabel,
} from '../../lib/relationship-intel/channels'
import { summarizeRelationshipEvidence } from '../../lib/relationship-intel/relationship-context'
import type { CommsMomentRecord, CommsRepository, CommsSourceRecord } from './repository'
import {
  COMMS_MAX_PAGE_SIZE,
  COMMS_MOMENT_LIMIT,
  COMMS_OPERATIONS,
  COMMS_PAGE_SIZE,
  type CommsAggregateDto,
  type CommsMomentDto,
  type CommsOperationMap,
  type CommsSourceDto,
} from './types'

function clampInt(value: number | undefined, min: number, max: number, fallback: number): number {
  if (value === undefined || !Number.isFinite(value)) return fallback
  return Math.min(Math.max(Math.trunc(value), min), max)
}

/**
 * A source row, under the pane's SOURCE_SLOTS vocabulary. FaceTime and Phone stay
 * separate rows: they are different things and the pane deliberately excludes
 * FaceTime from the Phone slot.
 */
function toSourceDto(record: CommsSourceRecord): CommsSourceDto {
  const channel = sourceChannelFor(record.source)
  return { ...record, channel, label: sourceChannelLabel(channel) }
}

/**
 * A moment, under the pane's channelMeta vocabulary.
 *
 * The channel is the interaction's OWN channel — already canonical in the
 * warehouse and never re-derived from the source system (that is what produced
 * unmappable values). The one exception is a call: both phone and FaceTime store
 * channel 'call', so the phone-vs-FaceTime delineation the intake recorded
 * (source_system apple_facetime / event_type facetime_call) is read back here.
 */
function toMomentDto(record: CommsMomentRecord): CommsMomentDto {
  const stored = momentChannelFor(record.channel)
  const channel = stored === 'call' && isFaceTimeInteraction(record) ? 'facetime' : stored
  return {
    id: record.id,
    channel,
    sourceSystem: record.sourceSystem,
    direction: record.direction,
    occurredAt: record.occurredAt,
    title: record.title,
    summary: record.summary,
  }
}

/** Panel order follows the pane's slot order, so Phone sits above FaceTime. */
const SOURCE_ORDER = ['call', 'imessage', 'whatsapp', 'email', 'facetime', 'calendar', 'other']

function sourceOrder(channel: string): number {
  const index = SOURCE_ORDER.indexOf(channel)
  return index === -1 ? SOURCE_ORDER.length : index
}

/**
 * COMMS — communication history for a canonical Person.
 *
 * The screen used to assemble this itself from `db/clients.ts` and
 * `db/contact-history.ts`, which is why the channel vocabulary lived in three
 * places at once. It lives here now: every source is mapped through
 * channelForSource before the UI sees it, so `apple_calls` renders as Call and
 * `apple_facetime` as Meeting instead of falling through to a generic label.
 *
 * Reads warehouse read models only. Never an ODS (l_) table.
 */
export class CommsService extends BaseService<CommsOperationMap> {
  readonly domain = 'comms'
  readonly version = '1'
  readonly description =
    'Owns communication history for a canonical Person and the channel vocabulary the CRM pane renders.'
  protected readonly operations: ServiceOperationDefinitions<CommsOperationMap>

  constructor(
    private readonly repository: CommsRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [COMMS_OPERATIONS.PANEL]: {
        kind: 'query',
        description: 'Aggregate header, one row per source and the newest moments for one Person.',
        authorization: 'comms.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => {
          const limit = clampInt(request.momentLimit, 0, COMMS_MAX_PAGE_SIZE, COMMS_MOMENT_LIMIT)
          const [sources, evidence, lastContact, page] = await Promise.all([
            this.repository.sources(request.personId),
            this.repository.evidence(request.personId),
            this.repository.lastContact(request.personId),
            this.repository.moments(request.personId, limit, 0),
          ])
          return {
            personId: request.personId,
            aggregate: this.buildAggregate(evidence, sources, lastContact),
            sources: sources
              .map(toSourceDto)
              .sort(
                (a, b) =>
                  sourceOrder(a.channel) - sourceOrder(b.channel) || a.source.localeCompare(b.source),
              ),
            moments: page.moments.map(toMomentDto),
            momentCount: page.total,
          }
        },
      },
      [COMMS_OPERATIONS.TIMELINE]: {
        kind: 'query',
        description: 'The canonical interaction timeline for one Person, newest first.',
        authorization: 'comms.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => {
          const pageSize = clampInt(request.pageSize, 1, COMMS_MAX_PAGE_SIZE, COMMS_PAGE_SIZE)
          const page = clampInt(request.page, 1, Number.MAX_SAFE_INTEGER, 1)
          const result = await this.repository.moments(request.personId, pageSize, (page - 1) * pageSize)
          return {
            personId: request.personId,
            moments: result.moments.map(toMomentDto),
            total: result.total,
            page,
            pageSize,
          }
        },
      },
    }
  }

  /**
   * The header is summarized in exactly one place, over neutral evidence rows,
   * so it cannot drift from the per-source rows the read model provides. Bulk and
   * service evidence is excluded from meaningful contact by the summarizer.
   */
  private buildAggregate(
    evidence: Parameters<typeof summarizeRelationshipEvidence>[0],
    sources: CommsSourceRecord[],
    lastContact: { at: string | null; label: string | null },
  ): CommsAggregateDto {
    const summary = summarizeRelationshipEvidence(evidence)
    const activeChannels = new Set(
      sources.filter((source) => source.totalCount > 0).map((source) => sourceChannelFor(source.source)),
    )
    return {
      observedCount: summary.observedCommunicationCount,
      inboundCount: summary.inboundCount,
      outboundCount: summary.outboundCount,
      twoWay: summary.twoWay,
      firstObservedAt: summary.firstObservedAt,
      lastInboundAt: summary.lastInboundAt,
      lastOutboundAt: summary.lastOutboundAt,
      lastContactAt: lastContact.at ?? summary.lastMeaningfulContactAt,
      lastContactLabel: lastContact.label,
      activeSourceCount: activeChannels.size,
      sourceCount: COMMS_SOURCE_SLOT_COUNT,
    }
  }

  invariants() {
    return [
      'Comms reads warehouse read models; an ODS (l_) table is never a source.',
      'Every source is mapped to a canonical channel before the UI sees it.',
      'Bulk and service evidence never refreshes meaningful last contact.',
    ] as const
  }
}