// ---------------------------------------------------------------------------
// Agent work command — single-worker dispatch between the authoritative
// (PRODUCTION) Story Board and the coding agent.
//
//   pnpm agent:work                          claim next Ready item, begin the
//                                            story run, print the spec, exit
//   pnpm agent:work --finish <workItemId> --result <outcome> --completion <n>
//        [--notes <text>] [--commit <hash>] [--tests <text>]
//                                            record the finished run + mark
//                                            the work item Done
//   pnpm agent:work --error <workItemId> --error-text <text>
//                                            mark the work item Error
//
// Contract (Story Board work-queue story):
//   - always targets the PRODUCTION control-plane database (refuses otherwise)
//   - claims AT MOST ONE Ready work item per invocation; exits cleanly with
//     "no work" when the queue is empty or another item is already active
//   - displays the selected story execution specification
//   - transitions it into the existing run lifecycle (story -> In Progress,
//     run created with an immutable spec snapshot, work item -> Running)
//   - NEVER loops into a second story in one invocation
//   - the coding agent implements/tests against DEV, then records the result
//     via --finish (story/run/result + work item Done) or --error
//   - commits repository changes itself; never pushes
// ---------------------------------------------------------------------------

import { execSync } from 'node:child_process'

async function main(): Promise<void> {
  if ((process.env.APP_ENV ?? 'development') !== 'production') {
    console.error(
      'agent:work must target the PRODUCTION control plane. Run with APP_ENV=production (pnpm agent:work).',
    )
    process.exit(2)
  }

  const args = process.argv.slice(2)
  const command = args[0]

  if (command === '--finish' || command === '--error') {
    await runFinishCommand(command, args.slice(1))
    return
  }
  if (command && command.startsWith('-')) {
    console.error('Unknown option:', command)
    process.exit(2)
  }

  await runClaimCommand()
}

async function runClaimCommand(): Promise<void> {
  const {
    claimNextAgentWork,
    beginAgentWorkRun,
  } = await import('../db/agent-work')
  const { workstreamName } = await import('../lib/storyboard-data')

  const claim = await claimNextAgentWork(process.env.AGENT_WORKER_ID ?? 'coding-agent')
  if (!claim) {
    console.log('no work')
    return
  }

  const { workItem, story } = claim
  console.log('claimed', workItem.id, '->', story.id)
  console.log('story:', story.id, '—', story.title)
  console.log('workstream:', workstreamName(story.workstream), '| priority:', story.priority)

  // Begin the run lifecycle: story -> In Progress, run + spec snapshot,
  // work item -> Running with story_run_id.
  const begun = await beginAgentWorkRun(workItem.id)

  console.log('work item:', begun.workItem.id, 'state:', begun.workItem.state)
  console.log('run:', begun.workItem.storyRunId)

  console.log('--- story specification ---')
  const spec = [
    ['Goal', story.goal],
    ['Human notes', story.notes],
    ['Dependencies', story.dependencies],
    ['Preconditions', story.preconditions],
    ['Architect brief', story.architectBrief],
    ['Context refs', story.contextRefs],
    ['Acceptance criteria', story.acceptanceCriteria],
    ['Postconditions', story.postconditions],
  ] as const
  for (const [label, value] of spec) {
    console.log(`${label}: ${value ?? '(none)'}`)
  }
  console.log('---')
  console.log(
    'execute this story (at most one per invocation); then record the result with:',
  )
  console.log(
    `pnpm agent:work --finish ${begun.workItem.id} --result <Complete|Partial|Blocked|Failed|Deferred|Hold> --completion <0-100> --notes "<execution notes>" --commit <hash> --tests "<tests summary>"`,
  )
}

async function runFinishCommand(
  command: string,
  args: string[],
): Promise<void> {
  const { finishAgentWork, failAgentWork } = await import('../db/agent-work')

  function value(flag: string): string | undefined {
    const index = args.indexOf(flag)
    return index >= 0 ? args[index + 1] : undefined
  }

  const workItemId = args[0]
  if (!workItemId) {
    console.error(`usage: pnpm agent:work ${command} <workItemId> ...`)
    process.exit(2)
  }

  if (command === '--error') {
    const errorText = value('--error-text')
    if (!errorText) {
      console.error('--error requires --error-text <text>')
      process.exit(2)
    }
    const item = await failAgentWork(workItemId, errorText)
    console.log('work item', item.id, '->', item.state, '(Error)')
    return
  }

  const resultStatus = value('--result')
  const completionText = value('--completion')
  if (!resultStatus || !completionText) {
    console.error('--finish requires --result <outcome> --completion <0-100>')
    process.exit(2)
  }
  const completion = Number(completionText)
  if (!Number.isInteger(completion) || completion < 0 || completion > 100) {
    console.error('--completion must be an integer 0-100')
    process.exit(2)
  }

  const finished = await finishAgentWork(workItemId, {
    resultStatus,
    completion,
    notes: value('--notes') ?? '',
    commitHash: value('--commit') ?? null,
    testsSummary: value('--tests') ?? null,
  })

  console.log('work item', finished.workItem.id, '->', finished.workItem.state)
  console.log('run result:', finished.run && (finished.run as { resultStatus?: string }).resultStatus)
  console.log('story status:', finished.story.status, '| completion:', finished.story.completion)
  if (value('--commit')) {
    console.log('commit:', value('--commit'))
  }
}

void main()
