// ---------------------------------------------------------------------------
// PROBE — the learn loop's de-dupe, against a real Postgres. NET ZERO, DEV by default.
//
//   APP_ENV=dev node --env-file=.env.local --import tsx scripts/probe-learn-dedupe.ts
//
// The unit test proves the DECISION does not duplicate. This proves the failure mode that only a database
// can catch: two worker passes overlapping, both deciding to file, both inserting. Migration 181 answers
// with a partial unique index over the open states, so the second insert is refused by Postgres rather
// than by a check that happened a millisecond earlier in some other process.
//
// It also proves the index does NOT get in the way of normal dispatch: a story whose work item has a null
// learn_pattern_key is untouched by it.
//
// The probe creates two throwaway stories, files both, and deletes everything it made.
// ---------------------------------------------------------------------------

import { withdrawQueuedAgentWork } from '@/legacy/db/agent-work'
import { listOpenLearnPatternKeys, openReadyLearnItem } from '@/legacy/db/forge-learn'
import { createStoryboardStory, setStoryboardStatus } from '@/legacy/db/storyboard'
import { describeControlPlane } from '@/lib/execution-target'
import { interactiveSql } from '@/lib/neon-interactive'

const STORY_A = 'LEARN-PROBE-A'
const STORY_B = 'LEARN-PROBE-B'
const PATTERN = 'probe-dedupe-pattern'

async function cleanup(): Promise<void> {
  await interactiveSql`delete from agent_work_item where story_id in (${STORY_A}, ${STORY_B})`
  await interactiveSql`delete from forge_batch_item where story_id in (${STORY_A}, ${STORY_B})`
  await interactiveSql`delete from storyboard_story where id in (${STORY_A}, ${STORY_B})`
}

async function createProbeStory(id: string): Promise<void> {
  await createStoryboardStory({
    id,
    workstream: 'ENGINEERING',
    title: `learn dedupe probe ${id}`,
    priority: 'Low',
    status: 'Planned',
    notes: 'throwaway row created by probe-learn-dedupe.ts; deleted by the same probe',
    batch: null,
    goal: null,
    scope: null,
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: null,
    acceptanceCriteria: null,
    postconditions: null,
    completion: 0,
    rollup: false,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
  })
}

function check(label: string, passed: boolean, detail: string): boolean {
  console.log(`  ${passed ? 'ok  ' : 'FAIL'} ${label} — ${detail}`)
  return passed
}

async function main(): Promise<void> {
  const plane = describeControlPlane()
  console.log(`probe-learn-dedupe — control plane: APP_ENV=${plane.appEnv} → ${plane.target}`)
  if (plane.target !== 'dev') {
    console.log('REFUSING: this probe creates and deletes stories. Run it with APP_ENV=dev.')
    process.exitCode = 1
    return
  }

  let ok = true
  await cleanup()
  try {
    await createProbeStory(STORY_A)
    await createProbeStory(STORY_B)

    // 1. The first filing succeeds and the pattern becomes open.
    const first = await openReadyLearnItem({ storyId: STORY_A, patternKey: PATTERN, instructions: 'probe' })
    ok = check('the first learn item opens and is stamped', first === 1, `stamped ${first}`) && ok
    const open = await listOpenLearnPatternKeys()
    ok = check('the pattern reads as open', open.includes(PATTERN), open.join(', ') || '(none open)') && ok

    // 2. The second filing for the SAME pattern is refused by the database.
    //
    // The failure arrives NORMALIZED (kind `CONSTRAINT`, code `23505`) rather than as raw Postgres text:
    // the DatabaseGateway classifies driver errors at the repository boundary, which is the rule in
    // AGENTS.md. So the assertion is on the classification, not on a constraint name in a message.
    let refused = ''
    try {
      await openReadyLearnItem({ storyId: STORY_B, patternKey: PATTERN, instructions: 'probe' })
    } catch (error) {
      const normalized = error as { kind?: string; code?: string; message?: string }
      refused = `${normalized.kind ?? ''}/${normalized.code ?? ''} ${normalized.message ?? String(error)}`.trim()
    }
    ok = check(
      'a second open item for the same pattern is refused',
      /CONSTRAINT/.test(refused) || /23505/.test(refused),
      refused.slice(0, 70) || 'no error was thrown — the index is missing',
    ) && ok

    // 3. The index does not touch ordinary work: a null pattern key is outside it.
    await setStoryboardStatus(STORY_B, 'Ready')
    const ordinary = rows(await interactiveSql`
      select count(*)::int as c from agent_work_item
      where story_id = ${STORY_B} and learn_pattern_key is null
    `)
    const count = Number((ordinary[0] as { c?: number })?.c ?? 0)
    ok = check('an ordinary Ready item with no pattern key is unaffected', count === 1, `${count} ordinary item(s)`) && ok
  } finally {
    // NET ZERO: withdraw what was queued, then delete the probe's own rows.
    for (const story of [STORY_A, STORY_B]) {
      const { withdrawn, live } = await withdrawQueuedAgentWork(story)
      console.log(`  note  withdraw ${story}: withdrawn=${withdrawn} live=${live}`)
    }
    await cleanup()
  }

  const remaining = rows(await interactiveSql`
    select count(*)::int as c from storyboard_story where id in (${STORY_A}, ${STORY_B})
  `)
  const left = Number((remaining[0] as { c?: number })?.c ?? 0)
  ok = check('net zero', left === 0, `${left} probe row(s) remain`) && ok
  console.log(`\nprobe-learn-dedupe — ${ok ? 'PASS' : 'FAIL'}`)
  if (!ok) process.exitCode = 1
}

/** The Neon-shaped row array, without reaching into the driver's types. */
function rows(result: unknown): Array<Record<string, unknown>> {
  return Array.isArray(result) ? (result as Array<Record<string, unknown>>) : []
}

void main()
