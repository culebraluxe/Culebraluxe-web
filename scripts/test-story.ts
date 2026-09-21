#!/usr/bin/env node
// ---------------------------------------------------------------------------
// RUN THE FENCES, NOT THE WORLD.
//
// ~3200 tests live across nine suites. The engine's QA already runs ONLY a story's frozen assay
// commands, but a human verifying afterwards had no equivalent: the cheap option was a full suite and
// the expensive option was a full suite twice. This runs exactly what a story, a batch or a sprint
// declares as its proof — nothing else — and it de-duplicates, because a batch of stories often
// proves itself with the same fences.
//
//   node --import tsx --env-file=.env.local scripts/test-story.ts <STORY-ID>
//   node --import tsx --env-file=.env.local scripts/test-story.ts --batch 98
//   node --import tsx --env-file=.env.local scripts/test-story.ts --sprint 99
//
// THE BUDGET (docs/agent/TEST-BUDGET.md):
//   per story        → this, i.e. the story's own fence
//   per suite done   → that one suite (test:forge:engine, test:app, …)
//   per batch deploy → pnpm test:deploy-gate (parity + the two broad suites), run ONCE
//
// A story with no declared fence is reported as such rather than quietly passing: a story whose proof
// is missing is a story nobody can verify.
// ---------------------------------------------------------------------------
import { spawnSync } from 'node:child_process'

const args = process.argv.slice(2)
const flag = (name: string): string | null => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 ? args[i + 1] ?? null : null
}
const storyId = args.find((a) => !a.startsWith('--') && !/^\d+$/.test(a)) ?? null
const batch = flag('batch')
const sprint = flag('sprint')

if (!storyId && !batch && !sprint) {
  console.error(
    [
      'usage: test-story <STORY-ID> | --batch <n> | --sprint <n>',
      '  runs the frozen assay commands (the fences) declared by those stories, de-duplicated',
    ].join('\n'),
  )
  process.exit(2)
}

type Row = { id: string; assay_commands: string | null }

/** A fence is the backticked command on its own line: "- `node --import tsx --test path`". */
function commandsOf(text: string | null): string[] {
  if (!text) return []
  const found: string[] = []
  for (const raw of text.split(/\r?\n/)) {
    const match = /`([^`]+)`/.exec(raw)
    const command = match?.[1]?.trim()
    if (command && /^[a-z]/.test(command)) found.push(command)
  }
  return found
}

async function main(): Promise<void> {
  const { sql } = await import('@/legacy/db/client')
  const rows = (await (storyId
    ? sql`select id, assay_commands from storyboard_story where id = ${storyId}`
    : sql`select id, assay_commands from storyboard_story where batch = ${Number(sprint ?? batch)} order by id`)) as Row[]

  if (rows.length === 0) {
    console.error(`no story matched (${storyId ?? `batch ${batch ?? sprint}`})`)
    process.exit(1)
  }

  const owners = new Map<string, string[]>()
  for (const row of rows) {
    for (const command of commandsOf(row.assay_commands)) {
      const list = owners.get(command) ?? []
      list.push(row.id)
      owners.set(command, list)
    }
  }

  const declared = rows.filter((r) => commandsOf(r.assay_commands).length > 0).length
  console.log(
    `fences for ${rows.length} story/stories: ${owners.size} distinct command(s) — ` +
      `${declared} with a declared fence, ${rows.length - declared} without`,
  )
  for (const row of rows) {
    if (commandsOf(row.assay_commands).length === 0) console.log(`  ! ${row.id} declares no fence`)
  }

  let failures = 0
  for (const [command, stories] of owners) {
    console.log(`\n$ ${command}\n  proves ${stories.join(', ')}`)
    const result = spawnSync(command, { shell: true, stdio: 'inherit', env: process.env })
    if (result.status !== 0) {
      failures += 1
      console.log(`  x FAILED (exit ${String(result.status)})`)
    }
  }

  console.log(`\n${owners.size - failures}/${owners.size} fence(s) passed`)
  process.exit(failures === 0 ? 0 : 1)
}

main().catch((error) => {
  console.error(String(error instanceof Error ? error.message : error))
  process.exit(1)
})
