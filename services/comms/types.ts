import type { CommsMomentChannel, CommsSourceChannel } from '../../lib/relationship-intel/channels'
import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'

// ---------------------------------------------------------------------------
// COMMS — the Person -> screen contract for communication history.
//
// This is the layer the CRM Client pane reads. It owns three things the screen
// used to assemble itself from repositories:
//   * the aggregate header (counts, first/last, active sources)
//   * one row per communication source, under a CANONICAL channel name
//   * the canonical interaction timeline, paginated
// It never reads an ODS (l_) table: the warehouse read models are the floor.
// ---------------------------------------------------------------------------

export type CommsAggregateDto = {
  observedCount: number
  inboundCount: number
  outboundCount: number
  twoWay: boolean
  firstObservedAt: string | null
  lastInboundAt: string | null
  lastOutboundAt: string | null
  lastContactAt: string | null
  /** Pre-formatted by the read model (for example "Sep 11, 2026"). */
  lastContactLabel: string | null
  activeSourceCount: number
  sourceCount: number
}

export type CommsSourceDto = {
  /** The raw warehouse source, kept for traceability (apple_calls, apple_facetime, ...). */
  source: string
  /**
   * The source ROW the pane renders. FaceTime and Phone are deliberately separate
   * rows, so the two Apple call sources are NOT merged here.
   */
  channel: CommsSourceChannel
  label: string
  firstObservedAt: string | null
  lastContactAt: string | null
  lastInboundAt: string | null
  lastOutboundAt: string | null
  inboundCount: number
  outboundCount: number
  totalCount: number
  twoWay: boolean
  lastDirection: 'inbound' | 'outbound' | null
  lastContext: string | null
  lastContextAt: string | null
  lastContextType: string | null
  lastContextDirection: 'inbound' | 'outbound' | null
}

export type CommsMomentDto = {
  id: string
  /** Canonical moment channel from interaction.channel — the pane's channelMeta vocabulary. */
  channel: CommsMomentChannel | null
  /** The raw interaction source system, kept for traceability. */
  sourceSystem: string | null
  direction: 'inbound' | 'outbound' | null
  occurredAt: string
  title: string | null
  summary: string | null
}

export type CommsPanelDto = {
  personId: string
  aggregate: CommsAggregateDto
  sources: CommsSourceDto[]
  moments: CommsMomentDto[]
  /** Total canonical interactions, so the pane can size its "View all". */
  momentCount: number
}

export type CommsTimelineDto = {
  personId: string
  moments: CommsMomentDto[]
  total: number
  page: number
  pageSize: number
}

export type GetCommsPanelRequest = { personId: string; momentLimit?: number }
export type GetCommsTimelineRequest = { personId: string; page?: number; pageSize?: number }

export const COMMS_MOMENT_LIMIT = 10
export const COMMS_PAGE_SIZE = 20
export const COMMS_MAX_PAGE_SIZE = 100

export const COMMS_OPERATIONS = {
  PANEL: 'comms.panel',
  TIMELINE: 'comms.timeline',
} as const

export type CommsOperationMap = {
  'comms.panel': { request: GetCommsPanelRequest; response: CommsPanelDto }
  'comms.timeline': { request: GetCommsTimelineRequest; response: CommsTimelineDto }
}

export type CommsOperationName = ServiceOperationName<CommsOperationMap>
export type CommsEnvelope<K extends CommsOperationName = CommsOperationName> =
  ServiceEnvelopeFor<CommsOperationMap, K>
