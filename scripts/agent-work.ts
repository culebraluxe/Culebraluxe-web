// agent work command — see file history for the full ENG-18/20/21 contract.
// Forge V2: hydrate bare Ready envelopes, then claim one, then follow Smith→Assay.

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { buildAgentInvokerWorkspaces } from '../agent-runtime/invoker'

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
  if (command === '--progress') {
    await runProgressCommand(args.slice(1))
    return
  }
  if (command === '--cancel') {
    await runCancelCommand(args.slice(1))
    return
  }
  if (command === '--recover') {
    await runRecoverCommand(args.slice(1))
    return
  }
  if (command && command.startsWith('-')) {
    console.error('Unknown option:', command)
    process.exit(2)
  }

  await runClaimCommand()
}

function value(args: string[], flag: string): string | undefined {
  const index = args.indexOf(flag)
  return index >= 0 ? args[index + 1] : undefined
}

function packetFailedCommands(storyId: string): string[] {
  try {
    const markdown = readFileSync(
      join(process.cwd(), 'docs', 'agent', 'packets', `${storyId}.md`),
      'utf8',
    )
    // Keep Loop parsing out of the planner/runtime. This only reads the packet
    // evidence already named by the architect.
    const section = markdown.match(/^## Assay commands\s*$([\s\S]*?)(?=^##\s|\Z)/im)?.[1] ?? ''
    return section
      .split('\n')
      .map((line) => line.replace(/^\s*-\s*/, '').trim())
      .filter((line) => Boolean(line) && !line.startsWith('#'))
  } catch {
    return []
  }
}

async function normalizeAssayFinish(input: {
  storyId: string
  role: string | null
  resultStatus: string
  testsSummary: string | null
}): Promise<{
  failed: boolean
  resultStatus: string
  testsSummary: string | null
}> {
  if (input.role !== 'reviewer') {
    return {
      failed: false,
      resultStatus: input.resultStatus,
      testsSummary: input.testsSummary,
    }
  }

  const { assayFailureEvidence, isCleanAssayResult } = await import(
    '../agent-runtime/orchestrate-apply'
  )
  if (
    isCleanAssayResult({
      resultStatus: input.resultStatus,
      testsSummary: input.testsSummary,
    })
  ) {
    return {
      failed: false,
      resultStatus: input.resultStatus,
      testsSummary: input.testsSummary,
    }
  }

  return {
    failed: true,
    // Never hand Complete/100 to finishStoryRun for a known failed Assay.
    // The work attempt can still become Done; the story is repair, not shipped.
    resultStatus: 'Assay Failed',
    testsSummary: assayFailureEvidence({
      testsSummary: input.testsSummary,
      failedCommands: packetFailedCommands(input.storyId),
    }),
  }
}

async function applyAssayRepairState(input: {
  workItemId: string
  storyId: string
  role: string | null
  resultStatus: string
  testsSummary: string | null
}): Promise<string | null> {
  const normalized = await normalizeAssayFinish(input)
  if (!normalized.failed) return normalized.testsSummary

  const { getAgentWorkItem } = await import('../db/agent-work')
  const { setStoryboardStatus, updateStoryRunProgress } = await import('../db/storyboard')
  const item = await getAgentWorkItem(input.workItemId)

  // Autonomous adapters have already persisted their terminal run by the time
  // control returns here. Enrich that existing run rather than creating a new
  // one or inventing a planner run.
  if (item?.storyRunId && normalized.testsSummary) {
    await updateStoryRunProgress(item.storyRunId, {
      testsSummary: normalized.testsSummary,
      note: 'Assay failed: story retained for repair; grow not enqueued.',
    })
  }
  await setStoryboardStatus(input.storyId, 'Hold')
  return normalized.testsSummary
}

async function runClaimCommand(): Promise<void> {
  const {
    claimNextAgentWork,
    getActiveAgentWorkItem,
    listStaleAgentWork,
    validateAgentWorkLaunchConfig,
    rejectAgentWorkConfiguration,
    escalateAgentWorkFailure,
  } = await import('../db/agent-work')
  const { workstreamName } = await import('../lib/storyboard-data')
  const { executeClaimedAgentCommand } = await import('../agent-runtime/invoker')
  const { createAgentRuntimeRegistry } = await import('../agent-runtime/factory')
  const {
    SqlAgentWorkRepository,
    SqlAgentRunRepository,
  } = await import('../agent-runtime/repositories')
  const { interactiveSql } = await import('../lib/neon-interactive')
  const {
    assertExecutionTargetSafe,
    parseExecutionEnvironment,
    verifyWorkspaceEnvFile,
  } = await import('../lib/execution-target')
  const { resolve } = await import('node:path')
  const { runForgeHydrate, runForgeFollow } = await import('./forge-orchestrate-wake')

  const workerId = process.env.AGENT_WORKER_ID ?? 'coding-agent'
  const hydrated = await runForgeHydrate()
  if (hydrated.length) console.log('hydrated', hydrated.join(', '))

  const claim = await claimNextAgentWork(workerId)
  if (!claim) {
    const active = await getActiveAgentWorkItem()
    if (!active) {
      console.log('no work')
      return
    }
    const staleAfterMinutes = Number(
      process.env.AGENT_WORKER_STALE_AFTER_MINUTES ?? 60,
    )
    const stale = await listStaleAgentWork(staleAfterMinutes)
    if (stale.some((w) => w.id === active.id)) {
      console.log('no work — the active item is STALE:')
      console.log(`  ${active.id} (${active.state}) last heartbeat ${active.updatedAt}`)
      console.log(
        'mark it terminal and unblock the queue with: pnpm agent:work --recover',
      )
    } else {
      console.log(
        `no work — another item is already active: ${active.id} (${active.state})`,
      )
    }
    return
  }

  const { workItem, story } = claim
  console.log('claimed', workItem.id, '->', story.id)
  console.log('story:', story.id, '—', story.title)
  console.log('workstream:', workstreamName(story.workstream), '| priority:', story.priority)
  console.log(
    'command:', workItem.role ?? '(no role)', '|',
    workItem.modelProfile ?? '(no model profile)', '|',
    'target', workItem.executionEnvironment ?? '(no execution target)', '|',
    'policy', workItem.executionPolicy ?? '(no execution policy)',
  )

  const launchError = validateAgentWorkLaunchConfig(workItem)
  if (launchError) {
    console.error('launch guard:', launchError)
    console.error(
      `work item ${workItem.id} marked Error; story ${story.id} set to Hold. ` +
        'Set role/profile via Forge hydrate (flip Ready with a brief) or enqueue-lane.',
    )
    await rejectAgentWorkConfiguration(workItem.id, launchError)
    process.exit(1)
  }

  const target = parseExecutionEnvironment(process.env.EXECUTION_ENV, 'DEV')
  assertExecutionTargetSafe(target)
  verifyWorkspaceEnvFile(resolve(process.cwd()), target)

  const work = new SqlAgentWorkRepository(() => interactiveSql as any)
  const runs = new SqlAgentRunRepository(() => interactiveSql as any)
  const registry = createAgentRuntimeRegistry()
  const workspaces = buildAgentInvokerWorkspaces(workerId)
  if (workspaces) {
    console.log(
      'workspace execution: base ref',
      workspaces.baseRef,
      '| run id = work item id | worktrees root:',
      workspaces.worktreesRoot ?? 'default (../Culebraluxe-worktrees)',
    )
  } else {
    console.log('workspace execution: DISABLED (legacy shared-checkout path)')
  }

  try {
    const result = await executeClaimedAgentCommand(workerId, claim, {
      work,
      runs,
      registry,
      ...(workspaces ? { workspaces } : {}),
      // ENG-FORGE-V4-08: this durable poller enforces the execution-contract
      // gate at the final pre-execution boundary for Smith launches.
      enforceExecutionContract: true,
    })
    console.log('=== autonomous dispatch result ===')
    console.log('story:', result.storyId)
    console.log('role:', result.role, '| profile:', result.modelProfile, '| adapter:', result.runtimeAdapter)
    console.log('evidence:', result.evidence.resultStatus, result.evidence.completion + '%')
    console.log('external_run_id:', result.evidence.externalRunId)
    console.log('execution_target:', result.evidence.executionEnvironment ?? 'unset')
    console.log('commit:', result.evidence.commitHash ?? '(no worker commit — still at base)')

    const testsSummary = await applyAssayRepairState({
      workItemId: result.workItemId,
      storyId: result.storyId,
      role: result.role,
      resultStatus: result.evidence.resultStatus,
      testsSummary: result.evidence.testsSummary,
    })

    const followed = await runForgeFollow({
      storyId: result.storyId,
      finishedRole: result.role,
      resultStatus: result.evidence.resultStatus,
    })
    if (followed) console.log('followed with lane', followed)
    if (result.role === 'reviewer' && testsSummary !== result.evidence.testsSummary) {
      console.log('assay evidence:', testsSummary)
    }
  } catch (e) {
    console.error('dispatch escalation:', String((e as Error)?.message ?? e).slice(0, 2000))
    await escalateAgentWorkFailure(workItem.id, e)
    process.exit(1)
  }
}

async function runProgressCommand(args: string[]): Promise<void> {
  const { updateAgentWorkProgress } = await import('../db/agent-work')

  const workItemId = args[0]
  if (!workItemId) {
    console.error(
      'usage: pnpm agent:work --progress <workItemId> [--completion <0-100>] [--note <text>] [--tests <text>]',
    )
    process.exit(2)
  }
  const completionText = value(args, '--completion')
  const completion =
    completionText === undefined ? undefined : Number(completionText)
  if (
    completion !== undefined &&
    (!Number.isInteger(completion) || completion < 0 || completion > 100)
  ) {
    console.error('--completion must be an integer 0-100')
    process.exit(2)
  }

  const result = await updateAgentWorkProgress(workItemId, {
    completion,
    note: value(args, '--note'),
    testsSummary: value(args, '--tests') ?? null,
  })

  console.log('work item', result.workItem.id, 'state:', result.workItem.state)
  console.log('run:', result.run.id.slice(0, 12), '| completion:', result.run.completion, '%')
  if (result.run.notes) {
    console.log('last milestone:', result.run.notes.split('\n').pop())
  }
  if (result.run.testsSummary) {
    console.log('tests:', result.run.testsSummary)
  }
}

async function runCancelCommand(args: string[]): Promise<void> {
  const { cancelAgentWork } = await import('../db/agent-work')

  const workItemId = args[0]
  if (!workItemId) {
    console.error('usage: pnpm agent:work --cancel <workItemId> [--note <text>]')
    process.exit(2)
  }

  const item = await cancelAgentWork(workItemId, {
    note: value(args, '--note'),
  })
  console.log('work item', item.id, '->', item.state, '(Cancelled)')
  console.log('run terminated; story set to Hold. Re-queue by setting the story back to Ready.')
}

async function runRecoverCommand(args: string[]): Promise<void> {
  const { recoverStaleAgentWork } = await import('../db/agent-work')

  const staleAfterText = value(args, '--stale-after')
  const staleAfterMinutes = staleAfterText
    ? Number(staleAfterText)
    : Number(process.env.AGENT_WORKER_STALE_AFTER_MINUTES ?? 60)
  if (!Number.isInteger(staleAfterMinutes) || staleAfterMinutes <= 0) {
    console.error('--stale-after must be a positive integer (minutes)')
    process.exit(2)
  }

  const recovered = await recoverStaleAgentWork(staleAfterMinutes)
  if (recovered.length === 0) {
    console.log('no stale work to recover')
    return
  }
  for (const item of recovered) {
    console.log(
      'recovered',
      item.id,
      '->',
      item.state,
      '|',
      item.errorText ?? '',
    )
  }
}

async function runFinishCommand(
  command: string,
  args: string[],
): Promise<void> {
  const { finishAgentWork, failAgentWork, getAgentWorkItem } = await import('../db/agent-work')

  const workItemId = args[0]
  if (!workItemId) {
    console.error(`usage: pnpm agent:work ${command} <workItemId> ...`)
    process.exit(2)
  }

  if (command === '--error') {
    const errorText = value(args, '--error-text')
    if (!errorText) {
      console.error('--error requires --error-text <text>')
      process.exit(2)
    }
    const completionText = value(args, '--completion')
    const completion =
      completionText === undefined ? undefined : Number(completionText)
    if (
      completion !== undefined &&
      (!Number.isInteger(completion) || completion < 0 || completion > 100)
    ) {
      console.error('--completion must be an integer 0-100')
      process.exit(2)
    }
    const item = await failAgentWork(workItemId, errorText, {
      completion,
      note: value(args, '--note'),
      testsSummary: value(args, '--tests') ?? null,
    })
    console.log('work item', item.id, '->', item.state, '(Error)')
    return
  }

  const resultStatus = value(args, '--result')
  const completionText = value(args, '--completion')
  if (!resultStatus || !completionText) {
    console.error('--finish requires --result <outcome> --completion <0-100>')
    process.exit(2)
  }
  const completion = Number(completionText)
  if (!Number.isInteger(completion) || completion < 0 || completion > 100) {
    console.error('--completion must be an integer 0-100')
    process.exit(2)
  }

  const before = await getAgentWorkItem(workItemId)
  const normalized = await normalizeAssayFinish({
    storyId: before?.storyId ?? '',
    role: before?.role ?? null,
    resultStatus,
    testsSummary: value(args, '--tests') ?? null,
  })

  const finished = await finishAgentWork(workItemId, {
    resultStatus: normalized.resultStatus,
    completion,
    notes: value(args, '--notes') ?? '',
    commitHash: value(args, '--commit') ?? null,
    testsSummary: normalized.testsSummary,
  })

  if (normalized.failed) {
    const { setStoryboardStatus } = await import('../db/storyboard')
    await setStoryboardStatus(finished.workItem.storyId, 'Hold')
  }

  console.log('work item', finished.workItem.id, '->', finished.workItem.state)
  console.log('run result:', finished.run && (finished.run as { resultStatus?: string }).resultStatus)
  console.log(
    'story status:',
    normalized.failed ? 'Hold' : finished.story.status,
    '| completion:',
    finished.story.completion,
  )
  if (value(args, '--commit')) {
    console.log('commit:', value(args, '--commit'))
  }
  if (normalized.failed && normalized.testsSummary) {
    console.log('assay evidence:', normalized.testsSummary)
  }

  const { runForgeFollow } = await import('./forge-orchestrate-wake')
  const followed = await runForgeFollow({
    storyId: finished.workItem.storyId,
    finishedRole: finished.workItem.role,
    resultStatus: normalized.resultStatus,
  })
  if (followed) console.log('followed with lane', followed)
}

void main()
