// SYNC: docs/agent/ARCH-HANDOFF.md -> the ARCH-HANDOFF Story Board row.
//
//   node --env-file=.env.local --import tsx scripts/sync-arch-handoff.ts
//
// WHY THIS EXISTS: the architect_brief column of the ARCH-HANDOFF row had accumulated ~53 KB of
// sections appended by successive sessions — duplicated, superseded, and in two blocks stored with
// the two-character sequence backslash-n instead of newlines. A database text column cannot be
// reviewed or diffed, which is exactly why that damage went unnoticed for weeks. The canonical text
// now lives in a FILE in git; this script is the one-way pipe from the file to the row. Never edit
// the row by hand, and never edit the row instead of the file.
//
// Idempotent: if the row already matches the file, it writes nothing.
//
// NOTE: explicit PROD pool, not the application gateway — db/database-gateway.ts binds its executor
// at MODULE LOAD, so flipping APP_ENV inside a script body does not change the connection.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { createPoolExecutor } from './lib/pool-executor'
import { getStoryboardStory, updateStoryboardStory } from '../db/storyboard'

const STORY_ID = 'ARCH-HANDOFF'
const SOURCE_FILE = join(process.cwd(), 'docs/agent/ARCH-HANDOFF.md')
const PRESERVED = 'docs/agent/preserved/arch-handoff/ARCH-HANDOFF-pre-cleanup-2026-09-12.md'

const NOTES = `Canonical text lives in the repository at docs/agent/ARCH-HANDOFF.md — that file is the source, this row is generated from it by scripts/sync-arch-handoff.ts. Never hand-edit this row instead of the file.

NON-ROLLUP reference story: not executable backlog, must never create an agent work item, rollup=false.

Rewritten 2026-09-11/12: the row had accumulated ~53 KB appended by successive sessions with duplicated and superseded sections, and two blocks stored with literal backslash-n rather than newlines, which made the read-this-first invariants render as one unreadable line. Doctrine was preserved and re-ordered; superseded items are labelled rather than deleted; volatile commit hashes moved to a dated appendix. The pre-cleanup text is preserved at ${PRESERVED} so the rewrite stays auditable.

Read order for a new session: ARCH-HANDOFF, then DEEP1 (data pipeline), then SOP1 (factory health).`

async function main() {
  const url = process.env.DATABASE_URL_PROD
  if (!url) throw new Error('DATABASE_URL_PROD is not set (fail closed)')
  if (url === process.env.DATABASE_URL_DEV) throw new Error('PROD URL equals DEV URL (fail closed)')

  // The Neon branch token is only visible in the HOST, and a resolved-target diagnostic alone has
  // already lied in this repo (DEEP1 section 6). Print both and require them to differ.
  console.log(`prod host: ${new URL(url).host}`)
  console.log(
    `dev  host: ${process.env.DATABASE_URL_DEV ? new URL(process.env.DATABASE_URL_DEV).host : '(unset)'}`,
  )

  const brief = readFileSync(SOURCE_FILE, 'utf8')
  if (brief.length < 5000) {
    throw new Error(`refusing to sync: ${SOURCE_FILE} is only ${brief.length} chars (expected a real document)`)
  }
  // The DEFECT signature was a single enormous line carrying ~35 literal backslash-n sequences.
  // Prose that merely mentions the sequence (this document does, describing the defect) is fine, so
  // detect the signature rather than the character.
  const damagedLine = brief
    .split('\n')
    .find((line) => line.length > 1500 && line.includes('\\n'))
  if (damagedLine) {
    throw new Error(
      `refusing to sync: ${SOURCE_FILE} contains a ${damagedLine.length}-char line with literal backslash-n — that is the defect this file exists to prevent`,
    )
  }
  console.log(`source: ${SOURCE_FILE} (${brief.length} chars)`)

  const pool = createPoolExecutor(url)
  try {
    const story = await getStoryboardStory(STORY_ID, pool.execute)
    if (!story) throw new Error(`${STORY_ID} not found`)

    const current = story.architectBrief ?? ''
    if (current === brief && story.notes === NOTES) {
      console.log(`${STORY_ID}: already in sync with the file - nothing to write`)
      return
    }

    if (story.rollup) {
      throw new Error(`refusing to sync: ${STORY_ID} has rollup=true — a reference story must be rollup=false`)
    }

    await updateStoryboardStory(
      STORY_ID,
      {
        workstream: story.workstream,
        title: story.title,
        priority: story.priority,
        status: story.status,
        notes: NOTES,
        batch: story.batch,
        goal: story.goal,
        scope: story.scope,
        dependencies: story.dependencies,
        preconditions: story.preconditions,
        architectBrief: brief,
        contextRefs:
          'READ FIRST; architecture handoff; ultimate README; continuity record; then DEEP1 (data pipeline) then SOP1 (factory health); source of truth: docs/agent/ARCH-HANDOFF.md',
        acceptanceCriteria: story.acceptanceCriteria,
        postconditions: story.postconditions,
        completion: story.completion,
        rollup: false,
        plannedStartAt: story.plannedStartAt,
        actualStartAt: story.actualStartAt,
        completedAt: story.completedAt,
        operatingSurface: story.operatingSurface,
      },
      pool.execute,
    )

    console.log(`${STORY_ID}: synced`)
    console.log(`  architect_brief: ${current.length} -> ${brief.length} chars`)
    console.log(`  notes: ${(story.notes ?? '').length} -> ${NOTES.length} chars`)
    console.log(`  rollup=${story.rollup} preserved as false`)
  } finally {
    await pool.end()
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
