import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import {
  APPLE_MESSAGES_SOURCE,
  isGroupChatGuid,
  type AppleMessagesExport,
  type AppleMessagesMessage,
} from '../lib/relationship-intel/apple-messages'
import {
  mapAppleMessageToInteraction,
  resolveHandlePerson,
} from '../lib/relationship-intel/apple-message-materializer'
import { getRelationshipEvidenceRows } from './relationship-evidence'
import { upsertLatestInteraction } from './interactions'
import { landImessage, landImessageBatch } from './landing'
import { refreshClientReadModels } from './client-read-models'
import type { CreateInteractionInput } from '../lib/crm-types'
import type { LandedImessage } from './landing'

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
  /** Raw messages genuinely new in l_imessage (ODS keeps everything). */
  landed: number
  /** Warehouse source rows created. */
  inserted: number
  /** Warehouse source rows refreshed because a newer message arrived. */
  updated: number
  /** Source rows already carrying the newest message. */
  replayed: number
  skippedNoTimestamp: number
  skippedGroupChat: number
  errors: number
}

export type AppleMessageMaterializeProgress = Pick<
  AppleMessageMaterializeResult,
  'eventsSeen' | 'landed' | 'inserted' | 'updated' | 'replayed' | 'skippedNoTimestamp' | 'skippedGroupChat' | 'errors'
> & { processed: number }

export async function materializeAppleMessages(
  exportData: AppleMessagesExport,
  execute: QueryExecutor = sql,
  options: {
    refresh?: () => Promise<void>
    progressEvery?: number
    onProgress?: (progress: AppleMessageMaterializeProgress) => void
  } = {},
): Promise<AppleMessageMaterializeResult> {
  const refresh = options.refresh ?? refreshClientReadModels
  const progressEvery = Math.max(0, Math.trunc(options.progressEvery ?? 0))
  let processed = 0
  const result: AppleMessageMaterializeResult = {
    handles: exportData.handles.length,
    exactLinkedHandles: 0,
    unmatchedOrAmbiguousHandles: 0,
    eventsSeen: exportData.messages.length,
    landed: 0,
    inserted: 0,
    updated: 0,
    replayed: 0,
    skippedNoTimestamp: 0,
    skippedGroupChat: 0,
    errors: 0,
  }
  const reportProgress = () => {
    if (progressEvery === 0 || processed % progressEvery !== 0) return
    options.onProgress?.({
      processed,
      eventsSeen: result.eventsSeen,
      landed: result.landed,
      inserted: result.inserted,
      updated: result.updated,
      replayed: result.replayed,
      skippedNoTimestamp: result.skippedNoTimestamp,
      skippedGroupChat: result.skippedGroupChat,
      errors: result.errors,
    })
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

  // ---------------------------------------------------------------------------
  // ODS TAKES EVERYTHING, THE WAREHOUSE TAKES WHAT THE SCREEN NEEDS.
  //
  // Every message lands in l_imessage - that is the raw intake, and it is what makes
  // a lossy warehouse safe, because the detail is always re-derivable. Landing is
  // batched (measured: one round trip is 68.7ms, so per-row landing of 93,000
  // messages was hours of pure latency).
  //
  // The warehouse is cherry-picked. The Contact History pane shows ONE row per
  // source carrying the last-contact time and the last message, so a message channel
  // materializes ONE interaction per Person x source - the NEWEST message - not one
  // per message. Ami's 5,519 messages are 4 source rows.
  //
  // A landing batch that fails falls back to the single-row path for that batch only,
  // so one bad row can never lose the rest.
  // ---------------------------------------------------------------------------
  const CHUNK = 500
  let pendingLanding: LandedImessage[] = []

  const flushLanding = async (): Promise<void> => {
    if (pendingLanding.length === 0) return
    const batch = pendingLanding
    pendingLanding = []
    try {
      result.landed += await landImessageBatch(batch, execute)
    } catch {
      for (const item of batch) {
        try {
          const inserted = await landImessage(item, execute)
          if (inserted) result.landed += 1
        } catch {
          result.errors += 1
        }
      }
    }
  }

  /** The newest message seen for each Person x source, in memory only. */
  const latestBySource = new Map<string, CreateInteractionInput>()

  for (const handle of exportData.handles) {
    const resolved = resolveHandlePerson(byIdentityKey, handle.id)
    if (!resolved.ok) {
      result.unmatchedOrAmbiguousHandles += 1
      continue
    }
    result.exactLinkedHandles += 1

    const messages = byHandle.get(handle.rowid) ?? []
    for (const m of messages) {
      processed += 1
      // Group chats are never silently attributed to an individual person.
      if (isGroupChatGuid(m.chatGuid)) {
        result.skippedGroupChat += 1
        reportProgress()
        continue
      }
      if (!m.dateISO) {
        result.skippedNoTimestamp += 1
        reportProgress()
        continue
      }
      try {
        const input = mapAppleMessageToInteraction(
          m,
          resolved.canonicalPersonId,
          exportData.sourceAccount,
        )
        pendingLanding.push({
          sourceAccount: exportData.sourceAccount,
          sourceMessageId: m.guid,
          conversationId: m.chatGuid,
          handle: m.handleValue,
          direction: m.isFromMe ? 'outgoing' : 'incoming',
          service: m.service,
          sentAt: m.dateISO,
          text: m.text,
          raw: m,
        })
        if (pendingLanding.length >= CHUNK) await flushLanding()

        const key = `${resolved.canonicalPersonId}\u0000${input.channel}`
        const current = latestBySource.get(key)
        if (!current || String(input.occurredAt) > String(current.occurredAt)) {
          latestBySource.set(key, input)
        }
      } catch {
        result.errors += 1
      }
      reportProgress()
    }
  }

  await flushLanding()

  // The cherry-pick: one interaction per Person x source, keyed on the source rather
  // than the message so each run updates that row instead of appending another.
  for (const input of latestBySource.values()) {
    try {
      const outcome = await upsertLatestInteraction(
        {
          ...input,
          sourceSystem: APPLE_MESSAGES_SOURCE,
          sourceExternalId: `latest:${input.personId}:${input.channel}`,
        },
        execute,
      )
      if (outcome === 'inserted') result.inserted += 1
      else if (outcome === 'updated') result.updated += 1
      else result.replayed += 1
    } catch {
      result.errors += 1
    }
  }

  if (progressEvery > 0 && processed % progressEvery !== 0) {
    options.onProgress?.({
      processed,
      eventsSeen: result.eventsSeen,
      landed: result.landed,
      inserted: result.inserted,
      updated: result.updated,
      replayed: result.replayed,
      skippedNoTimestamp: result.skippedNoTimestamp,
      skippedGroupChat: result.skippedGroupChat,
      errors: result.errors,
    })
  }

  if (result.inserted > 0 || result.updated > 0) {
    await refresh()
  }

  return result
}
