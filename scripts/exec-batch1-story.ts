// Story Board execution-run CLI for this batch (production control plane).
// Reads production Story Board state via APP_ENV=production + .env.local.
// Usage:
//   start <storyId>
//   progress <runId> <completion> [--note text] [--tests text]
//   finish <runId> <resultStatus> <completion> [--notes text] [--tests text] [--commit hash]
import {
  startStoryRun,
  finishStoryRun,
  updateStoryRunProgress,
} from '../db/storyboard'

async function main() {
  const [cmd, ...rest] = process.argv.slice(2)

  if (cmd === 'start') {
    const storyId = rest[0]
    const { run, story } = await startStoryRun(storyId)
    console.log('RUN_ID=' + run.id)
    console.log(
      'STORY=' + JSON.stringify({ id: story.id, status: story.status, completion: story.completion }),
    )
    return
  }

  if (cmd === 'progress') {
    const runId = rest[0]
    const completion = Number(rest[1])
    const note = valueAfter(rest, '--note')
    const tests = valueAfter(rest, '--tests')
    const run = await updateStoryRunProgress(runId, { completion, note, testsSummary: tests })
    console.log('RUN=' + run.id)
    return
  }

  if (cmd === 'finish') {
    const runId = rest[0]
    const resultStatus = rest[1] as 'Complete' | 'Partial' | 'Blocked' | 'Failed' | 'Deferred' | 'Hold'
    const completion = Number(rest[2])
    const notes = valueAfter(rest, '--notes')
    const tests = valueAfter(rest, '--tests')
    const commitHash = valueAfter(rest, '--commit')
    const { run, story } = await finishStoryRun(runId, {
      resultStatus,
      completion,
      notes: notes ?? '',
      testsSummary: tests ?? null,
      commitHash: commitHash ?? null,
    })
    console.log(
      'STORY=' + JSON.stringify({ id: story.id, status: story.status, completion: story.completion }),
    )
    void run
    return
  }

  console.error(
    'usage: start <story> | progress <runId> <completion> [--note x] [--tests y] | finish <runId> <status> <completion> [--notes x] [--tests y] [--commit h]',
  )
  process.exit(1)
}

function valueAfter(args: string[], flag: string): string | undefined {
  const i = args.indexOf(flag)
  return i >= 0 && i + 1 < args.length ? args[i + 1] : undefined
}

main().catch((e) => {
  console.error(String(e).slice(0, 500))
  process.exit(1)
})
