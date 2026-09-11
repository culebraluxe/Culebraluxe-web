#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Merge Apple Contacts NOTES into a contacts-export.json artifact.
//
// WHY THIS EXISTS: macOS gates `CNContact.note` behind the
// `com.apple.developer.contacts.notes` entitlement, which a plain local Swift
// build does not have — so the exporter can only ever emit "". The Contacts app
// ITSELF can read notes (it owns them), and AppleScript reaches it without any
// entitlement. This step asks the app for `id<TAB>base64(note)` for every person
// that HAS a note, then writes that note onto the matching export contact.
//
// Join key: AppleScript `id of person` IS the ABPerson identifier, i.e. the
// export's `sourceId` and l_person.source_contact_id (verified 2026-09-10:
// E6B1694F-8F5E-40EB-97E0-AB8FDD3734EC:ABPerson on both sides).
//
// Notes are CONTEXT (operator memory aids), never a name and never an identity.
//
// Usage: node --import tsx scripts/merge-contacts-notes.ts --file <export.json> [--quiet]
// Reads:  <export.json>            (written by contact-export, validated by the loader)
// Writes: <export.json>            (same file, notes filled in; atomic replace)
// ---------------------------------------------------------------------------
import { spawnSync } from 'node:child_process'
import { readFileSync, writeFileSync, renameSync } from 'node:fs'

const argv = process.argv.slice(2)
const flag = (name: string): string | undefined => {
  const i = argv.indexOf(name)
  return i >= 0 && i + 1 < argv.length ? argv[i + 1] : undefined
}
const quiet = argv.includes('--quiet')

const file = flag('--file')
if (!file) {
  console.error('usage: merge-contacts-notes.ts --file <contacts-export.json>')
  process.exit(2)
}

/**
 * ONE AppleScript pass, using BULK property fetches.
 *
 * A `repeat with p in people ... note of p` loop costs one IPC round-trip PER PERSON:
 * measured 336s over 4,662 people. Fetching the whole property list once and iterating
 * it in-script is ~5s for the same data (measured: ids=2s, notes=2s, filter=1s).
 * Only people WITH a note pay the extra `do shell script` for base64.
 */
const APPLESCRIPT = `
set out to ""
tell application "Contacts"
  set ids to id of people
  set ns to note of people
  set total to count of ids
  repeat with k from 1 to total
    set n to item k of ns
    if n is not missing value and (n as text) is not "" then
      set b64 to do shell script "printf %s " & quoted form of (n as text) & " | base64"
      set out to out & (item k of ids) & tab & b64 & linefeed
    end if
  end repeat
end tell
return out
`

const started = Date.now()
const proc = spawnSync('osascript', ['-e', APPLESCRIPT], {
  encoding: 'utf8',
  maxBuffer: 64 * 1024 * 1024,
})
if (proc.status !== 0) {
  // Never fail the load: notes are context. Report and leave the artifact as-is.
  console.error(`[notes] AppleScript failed (exit ${proc.status}): ${(proc.stderr ?? '').trim().slice(0, 300)}`)
  console.error('[notes] contacts export left unchanged (notes absent for this run)')
  process.exit(0)
}

const notes = new Map<string, string>()
for (const line of String(proc.stdout ?? '').split('\n')) {
  const tab = line.indexOf('\t')
  if (tab <= 0) continue
  const id = line.slice(0, tab).trim()
  const b64 = line.slice(tab + 1).trim()
  if (!id || !b64) continue
  try {
    notes.set(id, Buffer.from(b64, 'base64').toString('utf8'))
  } catch {
    /* skip an undecodable note rather than losing the whole run */
  }
}

const batch = JSON.parse(readFileSync(file, 'utf8')) as {
  contacts?: Array<Record<string, unknown>>
}
const contacts = batch.contacts ?? []
let merged = 0
for (const c of contacts) {
  const id = typeof c.sourceId === 'string' ? c.sourceId : null
  if (!id) continue
  const note = notes.get(id)
  if (note && note.trim()) {
    c.note = note
    merged += 1
  }
}

const tmp = `${file}.tmp-notes`
writeFileSync(tmp, JSON.stringify(batch, null, 2))
renameSync(tmp, file)

if (!quiet) {
  const secs = ((Date.now() - started) / 1000).toFixed(1)
  console.log(
    `[notes] people with notes: ${notes.size}; merged onto export contacts: ${merged}/${contacts.length} (${secs}s)`,
  )
}
