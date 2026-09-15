// ---------------------------------------------------------------------------
// story:status — set a story's status through the sanctioned seam, with a dry run first.
//
//   pnpm story:status --ids ENG-FORGE-V5-23,ENG-FORGE-V5-24 --status Complete            # dry run
//   pnpm story:status --ids … --status Complete --apply --reason "why, in one line"
//
// Why this exists: the board is execution-control state, so a status change is a real change
// and there was no command for it — only the portal actions and a drag. The 2026-09-15 drag
// proof reopened five `Complete` stories, and repairing that meant either hand-written SQL or
// opening a browser. Neither belongs in a repair.
//
// Three deliberate choices:
//   - dry run by default: `--apply` is the only thing that writes, so a typo in --ids cannot
//     move real work;
//   - the status is validated against STORY_STATUSES, so a typo cannot invent a status the
//     board's buckets do not know (statusBucket() would silently read it as `open`);
//   - it goes through setStoryboardStatus(), NOT raw SQL: that is the same function the portal
//     actions use, so it keeps `completion = 100 on Complete` consistent and its failures reach
//     the durable capture seam as a tooling error rather than vanishing.
// ---------------------------------------------------------------------------

import { setStoryboardStatus, listStoryboardStories } from '@/db/storyboard'
import { STORY_STATUSES } from '@/lib/storyboard-data'

type Options = {
  ids: string[]
  status: string
  apply: boolean
  reason: string
  json: boolean
}

export function parseArgs(argv: string[]): Options {
  const options: Options = { ids: [], status: '', apply: false, reason: '', json: false }
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--ids') {
      options.ids = (argv[i + 1] ?? '')
        .split(',')
        .map((id) => id.trim())
        .filter(Boolean)
      i += 1
    } else if (arg === '--status') {
      options.status = argv[i + 1] ?? ''
      i += 1
    } else if (arg === '--reason') {
      options.reason = argv[i + 1] ?? ''
      i += 1
    } else if (arg === '--apply') {
      options.apply = true
    } else if (arg === '--format') {
      options.json = argv[i + 1] === 'json'
      i += 1
    }
  }
  return options
}

/** Everything that can be known before any write: the ids, the target, and the legal values. */
export function validate(options: Options): string[] {
  const problems: string[] = []
  if (options.ids.length === 0) problems.push('no --ids given')
  if (!options.status) problems.push('no --status given')
  else if (!(STORY_STATUSES as readonly string[]).includes(options.status)) {
    problems.push(`--status "${options.status}" is not a story status (${STORY_STATUSES.join(', ')})`)
  }
  if (options.apply && !options.reason.trim()) {
    problems.push('--apply requires --reason: a status change with no stated reason is unauditable')
  }
  return problems
}

async function main(): Promise<number> {
  const options = parseArgs(process.argv.slice(2))
  const problems = validate(options)
  if (problems.length > 0) {
    for (const problem of problems) console.error(`FAIL  ${problem}`)
    console.error('usage: pnpm story:status --ids A,B --status Complete [--apply --reason "…"] [--format json]')
    return 1
  }

  const all = await listStoryboardStories()
  const byId = new Map(all.map((story) => [String(story.id), story]))
  const targets = options.ids.map((id) => ({ id, story: byId.get(id) }))
  const missing = targets.filter((target) => !target.story)
  if (missing.length > 0) {
    for (const target of missing) console.error(`FAIL  story "${target.id}" was not found`)
    return 1
  }

  console.log(
    `story:status — target APP_ENV=${process.env.APP_ENV ?? '<unset>'}, ${options.apply ? 'APPLY' : 'DRY RUN'}` +
      `, ${targets.length} story(ies) → ${options.status}`,
  )

  const results: Array<{ id: string; from: string; to: string; changed: boolean }> = []
  for (const { id, story } of targets) {
    const from = String(story!.status)
    if (from === options.status) {
      results.push({ id, from, to: options.status, changed: false })
      continue
    }
    if (!options.apply) {
      results.push({ id, from, to: options.status, changed: true })
      continue
    }
    await setStoryboardStatus(id, options.status)
    results.push({ id, from, to: options.status, changed: true })
  }

  if (options.json) {
    console.log(JSON.stringify({ applied: options.apply, status: options.status, reason: options.reason, results }, null, 2))
  } else {
    for (const result of results) {
      console.log(
        `  ${result.id.padEnd(22)} ${result.from.padEnd(13)} → ${result.to.padEnd(13)}` +
          `${result.changed ? '' : ' (already there, untouched)'}`,
      )
    }
    const changed = results.filter((result) => result.changed).length
    console.log(
      `\n${options.apply ? 'changed' : 'would change'} ${changed} of ${results.length}` +
        `${options.apply ? '' : '  — re-run with --apply to write'}`,
    )
    if (options.apply && options.reason) console.log(`reason recorded: ${options.reason}`)
  }
  return 0
}

void main().then((code) => process.exit(code))
