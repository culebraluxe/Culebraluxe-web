import type { RelationshipEvidenceForContext } from '../../lib/relationship-intel/relationship-context'

// ---------------------------------------------------------------------------
// COMMS persistence boundary.
//
// Returns warehouse read-model rows only. The raw SOURCE is preserved as-is;
// turning a source into a channel is a domain rule and belongs to the service,
// not to the data layer.
// ---------------------------------------------------------------------------

export type CommsSourceRecord = {
  source: string
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

export type CommsMomentRecord = {
  id: string
  /** The canonical interaction channel, as stored. */
  channel: string | null
  sourceSystem: string | null
  direction: 'inbound' | 'outbound' | null
  occurredAt: string
  title: string | null
  summary: string | null
}

export type CommsMomentPage = {
  moments: CommsMomentRecord[]
  total: number
}

export interface CommsRepository {
  /** One row per communication source for a canonical Person. */
  sources(personId: string): Promise<CommsSourceRecord[]>
  /** Neutral evidence rows, so the aggregate is summarized in exactly one place. */
  evidence(personId: string): Promise<RelationshipEvidenceForContext[]>
  /** The read model's last-contact date plus its pre-formatted label. */
  lastContact(personId: string): Promise<{ at: string | null; label: string | null }>
  /** Canonical interactions, newest first. */
  moments(personId: string, limit: number, offset: number): Promise<CommsMomentPage>
}
