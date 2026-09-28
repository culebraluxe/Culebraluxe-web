// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
import { readFile } from 'node:fs/promises'

import { upsertAppleReminder } from '@/legacy/db/reminder-landing'

type EventKitReminderSnapshot = {
  reminderIdentifier?: unknown
  externalIdentifier?: unknown
  sourceAccount?: unknown
  listName?: unknown
  title?: unknown
  notes?: unknown
  startAt?: unknown
  dueAt?: unknown
  completed?: unknown
  completedAt?: unknown
  priority?: unknown
}

const snapshotPath =
  process.argv[2] || process.env.MAC_BRIDGE_REMINDERS_JSON || '/tmp/culebraluxe-reminders.json'

function text(value: unknown): string | null {
  return typeof value === 'string' ? value : null
}

async function main() {
  const rawText = await readFile(snapshotPath, 'utf8')
  const parsed: unknown = JSON.parse(rawText)
  if (!Array.isArray(parsed)) {
    throw new Error(`Reminders snapshot is not an array: ${snapshotPath}`)
  }

  let upserted = 0
  let rejected = 0
  let completed = 0
  let open = 0

  for (const candidate of parsed) {
    const raw = candidate as EventKitReminderSnapshot
    const sourceMessageId = text(raw.reminderIdentifier)?.trim()
    if (!sourceMessageId) {
      rejected += 1
      continue
    }

    const isCompleted = raw.completed === true
    await upsertAppleReminder({
      sourceAccount: text(raw.sourceAccount)?.trim() || 'apple-reminders',
      sourceMessageId,
      externalId: text(raw.externalIdentifier),
      listName: text(raw.listName),
      title: text(raw.title),
      notes: text(raw.notes),
      startsAt: text(raw.startAt),
      dueAt: text(raw.dueAt),
      completed: isCompleted,
      completedAt: text(raw.completedAt),
      priority: typeof raw.priority === 'number' ? raw.priority : null,
      raw,
    })

    upserted += 1
    if (isCompleted) completed += 1
    else open += 1
  }

  // Aggregate-only operational output: never print reminder titles/notes.
  console.log(
    JSON.stringify({
      source: 'apple_eventkit_reminders',
      snapshotPath,
      received: parsed.length,
      upserted,
      open,
      completed,
      rejected,
    }),
  )
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
