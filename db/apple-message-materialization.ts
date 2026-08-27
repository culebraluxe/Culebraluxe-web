import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import {
  APPLE_MESSAGES_SOURCE,
  type AppleMessagesExport,
  type AppleMessagesMessage,
} from '../lib/relationship-intel/apple-messages'
import {
  mapAppleMessageToInteraction,
  resolveHandlePerson,
} from '../lib/relationship-intel/apple-message-materializer'
import { getRelationshipEvidenceRows } from './relationship-evidence'
import { createInteraction } from './interactions'
import { refreshClientReadModels } from './client-read-models'

// ---------------------------------------------------------------------------
// REL-INTEL — Apple Messages EVENT materialization into canonical interaction.
//
// The missing final leg of the relationship-memory architecture:
//
//   Mac Messages DB -> export -> ODS evidence -> reconcile -> Person
//                          +-> EVENT materialization (THIS seam) -> interaction
//                                                                 -> mv_client_contact_history
//                                                                 -> Contact History panel
//
// Replay-safe: every row reuses the existing createInteraction seam, which is
// backed by the unique partial index on (source_system, source_external_id).
// Running the historical import twice inserts N then 0 duplicates.
//
// NO PRIVATE PROSE: the pure mapper (apple-message-materializer) sets
// title/summary to undefined; only channel / direction / occurred_at / source
// identity / minimal provenance are persisted.
//
// Only handles with an AUTHORITATIVE canonical linkage (review_state =
// 'exact_linked' with a canonical_person_id) are materialized. Ambiguous /
// unmatched / deferred handles are never silently attached to a person.
// ---------------------------------------------------------------------------

export type AppleMessageMaterializeResult = {
  handles: number
  exactLinkedHandles: number
  unmatchedOrAmbiguousHandles: number
  eventsSeen: number
  inserted: number
  replayed: number
  skippedNoTimestamp: number
  errors: number
}

export async function materializeAppleMessages(
  exportData: AppleMessagesExport,
  execute: QueryExecutor = sql,
  options: { refresh?: () => Promise<void> } = {},
): Promise<AppleMessageMaterializeResult> {
  const refresh = options.refresh ?? refreshClientReadModels
  const result: AppleMessageMaterializeResult = {
    handles: exportData.handles.length,
    exactLinkedHandles: 0,
    unmatchedOrAmbiguousHandles: 0,
    eventsSeen: exportData.messages.length,
    inserted: 0,
    replayed: 0,
    skippedNoTimestamp: 0,
    errors: 0,
  }

  // Current reconcile state per Apple handle. Only authoritative exact links
  // qualify; ambiguous/unmatched/deferred handles are never silently assigned.
  const evidence = await getRelationshipEvidenceRows(APPLE_MESSAGES_SOURCE, execute)
  const byIdentityKey = new Map<
    string,
    { reviewState: string; canonicalPersonId: string | null }
  >()
  for (const e of evidence) {
    byIdentityKey.set(e.sourceIdentityKey, {
      reviewState: e.reviewState,
      canonicalPersonId: e.canonicalPersonId,
    })
  }

  // Group source events by their handle so we only visit messages that belong
  // to a handle we are about to materialize.
  const byHandle = new Map<number, AppleMessagesMessage[]>()
  for (const m of exportData.messages) {
    if (m.handleId == null) continue
    const arr = byHandle.get(m.handleId) ?? []
    arr.push(m)
    byHandle.set(m.handleId, arr)
  }

  for (const handle of exportData.handles) {
    const resolved = resolveHandlePerson(byIdentityKey, handle.id)
    if (!resolved.ok) {
      result.unmatchedOrAmbiguousHandles += 1
      continue
    }
    result.exactLinkedHandles += 1

    const messages = byHandle.get(handle.rowid) ?? []
    for (const m of messages) {
      if (!m.dateISO) {
        result.skippedNoTimestamp += 1
        continue
      }
      try {
        const input = mapAppleMessageToInteraction(
          m,
          resolved.canonicalPersonId,
          exportData.sourceAccount,
        )
        const { created } = await createInteraction(input, execute)
        if (created) result.inserted += 1
        else result.replayed += 1
      } catch {
        result.errors += 1
      }
    }
  }

  if (result.inserted > 0) {
    await refresh()
  }

  return result
}
