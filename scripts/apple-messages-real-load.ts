// ---------------------------------------------------------------------------
// REL-INTEL — REAL Apple Messages -> DEV ODS proof loader.
//
// Reads the REAL export package at public/upload/data/apple-messages-export/
// (identities.jsonl + messages.jsonl), aggregates into one neutral
// integration_relationship_evidence row PER SOURCE HANDLE via the existing
// buildMessagesRelationshipEvidence, then proves the full pipeline against real
// data:
//   1. identity parse + phone/email normalization on actual handle values
//   2. inbound/outbound/two-way derived from real messages
//   3. upsert -> reconcile (exact/review/unmatched) -> read model
//   4. replay is idempotent (no duplicate ODS rows)
//   5. CREATE-IT exact-match path on a real handle (synthetic canonical person,
//      cleaned up afterward)
//
// Usage:
//   node --env-file=.env.local --import tsx scripts/apple-messages-real-load.ts slice
//   node --env-file=.env.local --import tsx scripts/apple-messages-real-load.ts full
//
// DEV only. Never touches production. Cleans up its own DEV rows + the
// synthetic canonical Person at the end.
// ---------------------------------------------------------------------------
import { readFileSync } from 'node:fs'
import { randomUUID } from 'node:crypto'
import {
  buildMessagesRelationshipEvidence,
  APPLE_MESSAGES_SOURCE,
  type AppleMessagesExport,
  type AppleMessagesHandle,
  type AppleMessagesMessage,
} from '../lib/relationship-intel/apple-messages'
import { normalizePhone } from '../lib/relationship-intel/normalize'
import {
  upsertRelationshipEvidence,
  recordReconcileDecision,
  getRelationshipEvidenceRows,
  getRelationshipEvidenceForPerson,
} from '../db/relationship-evidence'
import { reconcileEvidence } from '../lib/relationship-intel/reconcile'
import { createInMemoryPersonLookup, mapLimit } from '../lib/relationship-intel/inmemory-lookup'
import { createPoolExecutor } from './lib/pool-executor'

const url = (process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL) ?? ''
const dir = 'public/upload/data/apple-messages-export'
const MODE = process.argv[2] ?? 'slice' // 'slice' | 'full'
// `keep` preserves the ingested evidence + reconcile decisions in DEV instead
// of cleaning up (used to feed the Clients promotion/pagination path). Default
// (proof mode) still cleans up its own DEV rows + synthetic canonical Person.
const KEEP = process.argv[3] === 'keep'

const out = (...a: unknown[]) => console.log(...a)

function readLines(file: string): string[] {
  const raw = readFileSync(`${dir}/${file}`, 'utf8')
  return raw.split('\n').filter((l) => l.trim().length > 0)
}

function loadRealPackage() {
  const manifest = JSON.parse(readFileSync(`${dir}/manifest.json`, 'utf8')) as Record<string, unknown>
  const handles = readLines('identities.jsonl').map((l) => JSON.parse(l) as AppleMessagesHandle)
  const messages = readLines('messages.jsonl').map((l) => JSON.parse(l) as AppleMessagesMessage)
  return { manifest, handles, messages }
}

/** Representative bounded real slice (phone + email + unclassified + variety). */
function selectSlice(
  handles: AppleMessagesHandle[],
  messages: AppleMessagesMessage[],
): { handles: AppleMessagesHandle[]; messages: AppleMessagesMessage[] } {
  const stats = new Map<number, { in: number; out: number; att: number; text: number; noText: number }>()
  for (const m of messages) {
    if (m.handleId == null) continue
    let s = stats.get(m.handleId)
    if (!s) {
      s = { in: 0, out: 0, att: 0, text: 0, noText: 0 }
      stats.set(m.handleId, s)
    }
    if (m.isFromMe === 1) s.out += 1
    else s.in += 1
    if (m.hasAttachments === 1) s.att += 1
    if (m.text && String(m.text).length > 0) s.text += 1
    else s.noText += 1
  }

  const isPhone = (id: string) => /^\+?\d/.test(id)
  const isEmail = (id: string) => id.includes('@')
  const selected = new Set<number>()
  const addUpTo = (pred: (h: AppleMessagesHandle) => boolean) => {
    for (const h of handles) {
      if (selected.size >= 60) break
      if (selected.has(h.rowid)) continue
      if (pred(h)) selected.add(h.rowid)
    }
  }

  addUpTo((h) => isPhone(h.id) && (stats.get(h.rowid)?.in ?? 0) > 0 && (stats.get(h.rowid)?.out ?? 0) > 0)
  addUpTo((h) => isPhone(h.id) && (stats.get(h.rowid)?.in ?? 0) > 0 && (stats.get(h.rowid)?.out ?? 0) === 0)
  addUpTo((h) => isPhone(h.id) && (stats.get(h.rowid)?.out ?? 0) > 0 && (stats.get(h.rowid)?.in ?? 0) === 0)
  addUpTo((h) => isPhone(h.id) && (stats.get(h.rowid)?.att ?? 0) > 0)
  addUpTo((h) => isEmail(h.id))
  addUpTo((h) => !isPhone(h.id) && !isEmail(h.id))
  addUpTo((h) => (stats.get(h.rowid)?.text ?? 0) > 0 && (stats.get(h.rowid)?.noText ?? 0) > 0)

  const selHandles = handles.filter((h) => selected.has(h.rowid))
  const selMessages = messages.filter((m) => m.handleId != null && selected.has(m.handleId)).slice(0, 8000)
  return { handles: selHandles, messages: selMessages }
}

function deriveSourceAccount(messages: AppleMessagesMessage[]): string {
  for (const m of messages) {
    const acct = (m as unknown as { account?: string }).account
    if (acct && acct.includes('@')) return acct
  }
  return 'apple_messages_local'
}


async function runProof(
  exportData: AppleMessagesExport,
  execute: Awaited<ReturnType<typeof createPoolExecutor>>['execute'],
  label: string,
) {
  out(`\n==== ${label} ====`)
  out('handles in slice:', exportData.handles.length, '| messages in slice:', exportData.messages.length)

  const rows = buildMessagesRelationshipEvidence(exportData)
  out('BUILT neutral evidence rows (per source handle):', rows.length)

  // --- CREATE-IT: synthetic canonical Person for an EXACT-match proof on a real handle ---
  // Skipped in `keep` mode (the real ingest must not leave a synthetic proof
  // Person behind; the promotion path creates canonical Persons instead).
  // Pick a real phone handle whose normalized phone is NOT already a DEV
  // person_identity (so our proof Person is the sole exact match for it).
  let proofPhone: { raw: string; value: string } | null = null
  for (const h of exportData.handles) {
    if (KEEP) break
    if (!/^\+?\d/.test(h.id)) continue
    const n = normalizePhone(h.id)
    if (!n.ok) continue
    const existing = (await execute`
      select 1 from person_identity where identity_type = 'phone' and identity_value = ${n.value} limit 1
    `) as unknown[]
    if (existing.length === 0) {
      proofPhone = { raw: h.id, value: n.value }
      break
    }
  }
  const proofPersonId = randomUUID()
  if (proofPhone) {
    await execute`
      insert into person (id, display_name, role, status)
      values (${proofPersonId}, 'REL-INTEL Real Proof Person', 'buyer', 'new')
    `
    await execute`
      insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
      values (${proofPersonId}, 'phone', ${proofPhone.value}, ${APPLE_MESSAGES_SOURCE}, true)
    `
    out('CREATE-IT: canonical Person', proofPersonId, 'with phone', proofPhone.value, '(from real handle', proofPhone.raw + ')')
  } else {
    out('CREATE-IT: skipped (no unused phone handle in slice)')
  }

  const { lookup } = await createInMemoryPersonLookup(execute)
  out('lookup preloaded person_identity + explicit source links')

  // --- ingest + reconcile ---
  const tally: Record<string, number> = {}
  await mapLimit(rows, 16, async ({ evidence, fingerprint }) => {
    const id = await upsertRelationshipEvidence(evidence, fingerprint, undefined, execute)
    const decision = await reconcileEvidence(evidence, lookup)
    await recordReconcileDecision(id, decision, execute)
    tally[decision.reviewState] = (tally[decision.reviewState] ?? 0) + 1
  })
  out('RECONCILE tally:', JSON.stringify(tally))

  const persisted = (await execute`
    select count(*)::int as n from integration_relationship_evidence where source = ${APPLE_MESSAGES_SOURCE}
  `) as { n: number }[]
  out('PERSISTED apple_messages rows:', persisted[0].n)


  // --- replay idempotency ---
  const before = persisted[0].n
  await mapLimit(rows, 16, async ({ evidence, fingerprint }) => {
    await upsertRelationshipEvidence(evidence, fingerprint, undefined, execute)
  })
  const after = (await execute`
    select count(*)::int as n from integration_relationship_evidence where source = ${APPLE_MESSAGES_SOURCE}
  `) as { n: number }[]
  out('REPLAY: rows before=', before, 'after=', after[0].n, '| no duplicate=', after[0].n === before)

  // --- exact-match read proof ---
  if (proofPhone) {
    const exact = (await execute`
      select source_identity_key, review_state, canonical_person_id, match_method
      from integration_relationship_evidence
      where source = ${APPLE_MESSAGES_SOURCE} and canonical_person_id = ${proofPersonId}
    `) as Array<{ source_identity_key: string; review_state: string; canonical_person_id: string; match_method: string }>
    out('EXACT-MATCH read proof (handle -> canonical Person):', JSON.stringify(exact))
    const viaPerson = await getRelationshipEvidenceForPerson(proofPersonId, execute)
    out('getRelationshipEvidenceForPerson(proofPerson) ->', viaPerson.length, 'evidence row(s)')
  }

  // --- read model: all rows for source ---
  const allRead = await getRelationshipEvidenceRows(APPLE_MESSAGES_SOURCE, execute)
  out('getRelationshipEvidenceRows(apple_messages) ->', allRead.length, 'row(s)')
  const twoWayRows = allRead.filter((r) => r.isTwoWay === true)
  out('  of which two-way:', twoWayRows.length)

  // --- cleanup (skipped in `keep` mode so the real evidence stays in DEV) ---
  if (KEEP) {
    out('KEEP: preserving apple_messages evidence rows in DEV (no cleanup)')
  } else {
    if (proofPhone) {
      await execute`delete from person_identity where person_id = ${proofPersonId}`
      await execute`delete from person where id = ${proofPersonId}`
      out('CLEANUP: synthetic canonical Person', proofPersonId, 'removed')
    }
    await execute`delete from integration_relationship_evidence where source = ${APPLE_MESSAGES_SOURCE}`
    out('CLEANUP: apple_messages evidence rows removed from DEV')
  }
}


async function main() {
  const { execute, end } = createPoolExecutor(url)
  try {
    const { manifest, handles, messages } = loadRealPackage()
    const sourceAccount = deriveSourceAccount(messages)
    out('manifest generatedAt:', (manifest as { generatedAt?: string }).generatedAt)
    out('manifest timestampEncoding:', (manifest as { timestampEncoding?: string }).timestampEncoding)
    out('sourceAccount derived:', sourceAccount)
    out('REAL handles loaded:', handles.length, '| REAL messages loaded:', messages.length)

    // real stats (independent of ODS) for the report
    const textMsgs = messages.filter((m) => m.text && String(m.text).length > 0).length
    const inboundMsgs = messages.filter((m) => m.isFromMe === 0).length
    const outboundMsgs = messages.filter((m) => m.isFromMe === 1).length
    const attMsgs = messages.filter((m) => m.hasAttachments === 1).length
    const dated = messages.filter((m) => m.dateISO).length
    out('REAL stats -> text:', textMsgs, '| inbound:', inboundMsgs, '| outbound:', outboundMsgs, '| withAttachments:', attMsgs, '| withDateISO:', dated)

    if (MODE === 'slice') {
      const slice = selectSlice(handles, messages)
      await runProof(
        { sourceAccount, handles: slice.handles, messages: slice.messages },
        execute,
        'BOUNDED REAL SLICE',
      )
    } else {
      await runProof({ sourceAccount, handles, messages }, execute, 'FULL REAL EXPORT')
    }
  } finally {
    await end()
  }
}

main().then(() => process.exit(0)).catch((err) => { console.error(err); process.exit(1) })

