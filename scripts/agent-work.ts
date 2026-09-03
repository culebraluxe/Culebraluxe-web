// agent work command — see file history for the full ENG-18/20/21 contract.
// Forge V2: hydrate bare Ready envelopes, then claim one, then follow Smith→Assay.

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { buildAgentInvokerWorkspaces } from '../agent-runtime/invoker'
import { isAssayTerminalRole } from '../agent-runtime/candidate-assay-handoff'
import {
  postSlackNotification,
  type ForgeSlackContext,
} from '../agent-runtime/slack-notifier'

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

// ---------------------------------------------------------------------------
// ENG-FORGE-V4-11 — Slack mirror plumbing for the local Forge worker.
//
// Slack is a human cockpit only; Forge/Neon own truth. Every notification
// mirrors an outcome that is ALREADY durable in Neon and is strictly
// fail-open (agent-runtime/slack-notifier.ts never rejects), so none of these
// calls can gate or alter claim, execution, commit, Assay, or story
// completion. Events observed while the worker still has durable Forge steps
// ahead are buffered and flushed only after those steps have happened.
// ---------------------------------------------------------------------------

function slackIdentifiers(
  story: { id: string; title: string | null },
  item: { id: string; role: string | null; modelProfile: string | null },
): Pick<
  ForgeSlackContext,
  'storyId' | 'storyTitle' | 'workItemId' | 'role' | 'modelProfile'
> {
  return {
    storyId: story.id,
    storyTitle: story.title ?? null,
    workItemId: item.id,
    role: item.role,
    modelProfile: item.modelProfile,
  }
}

function mirrorForgeSlack(
  buffer: ForgeSlackContext[] | null,
  context: ForgeSlackContext,
): void {
  if (buffer) buffer.push(context)
  else void postSlackNotification(context)
}

async function flushForgeSlack(buffer: ForgeSlackContext[]): Promise<void> {
  // Awaited only at points where no durable Forge transition remains, so a
  // slow/unreachable Slack endpoint can never gate the run. postSlackNotification
  // never rejects; failures are logged by the notifier and skipped.
  for (const context of buffer) {
    await postSlackNotification(context)
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
  // ENG-FORGE-V4-10C: every terminal Assay role (reviewer + verifier) gets
  // the same fail-closed repair semantics — a verifier result must never be
  // normalized to Complete when its verification evidence failed.
  if (!isAssayTerminalRole(input.role)) {
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
  const { runForgeHydrate, runForgeFollow, runForgePublishAfterAssay } = await import('./forge-orchestrate-wake')

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
    // ENG-FORGE-V4-11 — mirror the terminal outcome (already durable: work
    // item Error + story Hold) before this process exits. Fail-open only.
    await postSlackNotification({
      event: 'lane-terminal',
      ...slackIdentifiers(story, workItem),
      resultStatus: 'Error',
      detail: `launch guard rejected the command: ${launchError}`,
    })
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

  // ENG-FORGE-V4-11 — mirror the claim to Slack (human cockpit). Fire-and-forget:
  // the notifier is fail-open and never rejects, and execution must never wait
  // on Slack. Events observed after dispatch are buffered and flushed once no
  // durable Forge transition remains.
  void postSlackNotification({
    event: 'lane-started',
    ...slackIdentifiers(story, workItem),
  })
  const slack: ForgeSlackContext[] = []

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

    // ENG-FORGE-V4-11 — classify the finished lane outcome and buffer its Slack
    // mirror. Nothing is posted yet: delivery is deferred until after follow and
    // publish so a slow/unreachable Slack endpoint can never gate them.
    const assayFailed =
      isAssayTerminalRole(result.role) &&
      testsSummary !== result.evidence.testsSummary
    const outcomeOk =
      !result.evidence.resultStatus ||
      /complete|success|pass/i.test(result.evidence.resultStatus)

    if (assayFailed) {
      mirrorForgeSlack(slack, {
        event: 'lane-terminal',
        ...slackIdentifiers(story, workItem),
        runtimeAdapter: result.runtimeAdapter,
        resultStatus: 'Assay Failed',
        detail:
          'terminal Assay failed its verification evidence; story set to Hold (repair, not Complete).',
      })
    } else if (outcomeOk) {
      mirrorForgeSlack(slack, {
        event: 'lane-completed',
        ...slackIdentifiers(story, workItem),
        runtimeAdapter: result.runtimeAdapter,
        resultStatus: result.evidence.resultStatus ?? null,
        completion: result.evidence.completion,
        commitHash: result.evidence.commitHash,
        externalRunId:
          result.evidence.externalRunId ?? workItem.externalRunId ?? null,
      })
    } else {
      mirrorForgeSlack(slack, {
        event: 'lane-terminal',
        ...slackIdentifiers(story, workItem),
        runtimeAdapter: result.runtimeAdapter,
        resultStatus: result.evidence.resultStatus ?? 'Failed',
        detail:
          'run ended without a clean result; failure text is in the Neon run evidence.',
      })
    }

    const followed = await runForgeFollow({
      storyId: result.storyId,
      finishedRole: result.role,
      resultStatus: result.evidence.resultStatus,
    })
    if (followed) {
      console.log('followed with lane', followed)
      mirrorForgeSlack(slack, {
        event: 'lane-follow',
        ...slackIdentifiers(story, workItem),
        toLane: followed,
      })
    }
    if (isAssayTerminalRole(result.role) && testsSummary !== result.evidence.testsSummary) {
      console.log('assay evidence:', testsSummary)
    }
    // ENG-FORGE-V4-11 — mirror the V4-10C follow Hold: a code-changing Smith
    // run that finished without a candidate commit holds the story so Assay
    // never verifies a fallback base. The Hold itself happened inside
    // runForgeFollow; this only observes the same inputs and outcome.
    if (
      !followed &&
      !assayFailed &&
      outcomeOk &&
      result.role === 'builder' &&
      !result.evidence.commitHash
    ) {
      mirrorForgeSlack(slack, {
        event: 'lane-terminal',
        ...slackIdentifiers(story, workItem),
        runtimeAdapter: result.runtimeAdapter,
        resultStatus: 'Hold',
        detail:
          'Smith produced no candidate commit for this code-changing run; story held so Assay never verifies a fallback base such as main.',
      })
    }

    // ENG-FORGE-V4-10B: a clean terminal Assay publishes the accepted Smith
    // candidate to origin/main from this OUTER Forge process (never from the
    // model sandbox). A publish conflict fails the story closed into Hold
    // inside runForgePublishAfterAssay; the candidate commit is preserved.
    const publishOutcome = await runForgePublishAfterAssay({
      storyId: result.storyId,
      finishedRole: result.role,
      resultStatus: result.evidence.resultStatus,
      testsSummary: result.evidence.testsSummary,
    })
    if (publishOutcome?.kind === 'published') {
      console.log(
        'published accepted candidate',
        publishOutcome.candidateCommit.slice(0, 12),
        'to origin/main as',
        publishOutcome.publishedMainHash.slice(0, 12),
      )
    } else if (publishOutcome?.kind === 'publish-conflict') {
      console.log('publish conflict — story set to Hold:', publishOutcome.detail)
      mirrorForgeSlack(slack, {
        event: 'lane-terminal',
        ...slackIdentifiers(story, workItem),
        runtimeAdapter: result.runtimeAdapter,
        resultStatus: 'Hold',
        detail: `publish conflict: ${publishOutcome.detail}`,
      })
    }

    // ENG-FORGE-V4-11 — every durable transition for this claim is complete;
    // flush the buffered Slack mirrors in causal order. Fail-open and never
    // throws, and there is no remaining Forge step it could delay.
    await flushForgeSlack(slack)
  } catch (e) {
    console.error('dispatch escalation:', String((e as Error)?.message ?? e).slice(0, 2000))
    await escalateAgentWorkFailure(workItem.id, e)
    // ENG-FORGE-V4-11 — mirror the escalation (already durable: work item
    // Error) before this process exits. Fail-open only.
    await postSlackNotification({
      event: 'lane-terminal',
      ...slackIdentifiers(story, workItem),
      resultStatus: 'Error',
      detail: String((e as Error)?.message ?? e),
    })
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
  // ENG-FORGE-V4-11 — mirror the cancellation (already durable: work item
  // Cancelled + story Hold) before this process exits. Fail-open only.
  await postSlackNotification({
    event: 'lane-terminal',
    storyId: item.storyId,
    workItemId: item.id,
    role: item.role ?? null,
    modelProfile: item.modelProfile ?? null,
    resultStatus: 'Cancelled',
    detail:
      'run cancelled by operator; story set to Hold. Re-queue by setting the story back to Ready.',
  })
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
    // ENG-FORGE-V4-11 — mirror the Error (already durable) before this process
    // exits. Fail-open only; detail is bounded and secret-redacted.
    await postSlackNotification({
      event: 'lane-terminal',
      storyId: item.storyId,
      workItemId: item.id,
      role: item.role ?? null,
      modelProfile: item.modelProfile ?? null,
      resultStatus: 'Error',
      detail: errorText,
    })
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

  // ENG-FORGE-V4-11 — classify the finished lane outcome and buffer its Slack
  // mirror. Delivery is deferred until after follow/publish so a slow or
  // unreachable Slack endpoint can never gate them.
  const slack: ForgeSlackContext[] = []
  const outcomeOk = !normalized.resultStatus || /complete|success|pass/i.test(normalized.resultStatus)
  const commitHash = (value(args, '--commit') ?? '').trim() || null
  if (normalized.failed) {
    mirrorForgeSlack(slack, {
      event: 'lane-terminal',
      storyId: finished.workItem.storyId,
      storyTitle: finished.story.title,
      workItemId: finished.workItem.id,
      role: finished.workItem.role,
      modelProfile: finished.workItem.modelProfile,
      resultStatus: 'Assay Failed',
      detail:
        'terminal Assay failed its verification evidence; story set to Hold (repair, not Complete).',
    })
  } else if (outcomeOk) {
    mirrorForgeSlack(slack, {
      event: 'lane-completed',
      storyId: finished.workItem.storyId,
      storyTitle: finished.story.title,
      workItemId: finished.workItem.id,
      role: finished.workItem.role,
      modelProfile: finished.workItem.modelProfile,
      runtimeAdapter: finished.workItem.runtimeAdapter ?? undefined,
      resultStatus: normalized.resultStatus ?? null,
      completion,
      commitHash,
      externalRunId: finished.workItem.externalRunId,
    })
  } else {
    mirrorForgeSlack(slack, {
      event: 'lane-terminal',
      storyId: finished.workItem.storyId,
      storyTitle: finished.story.title,
      workItemId: finished.workItem.id,
      role: finished.workItem.role,
      modelProfile: finished.workItem.modelProfile,
      resultStatus: normalized.resultStatus,
      detail:
        'run ended without a clean result; failure text is in the Neon run evidence.',
    })
  }

  const { runForgeFollow, runForgePublishAfterAssay } = await import('./forge-orchestrate-wake')
  const followed = await runForgeFollow({
    storyId: finished.workItem.storyId,
    finishedRole: finished.workItem.role,
    resultStatus: normalized.resultStatus,
  })
  if (followed) {
    console.log('followed with lane', followed)
    mirrorForgeSlack(slack, {
      event: 'lane-follow',
      storyId: finished.workItem.storyId,
      storyTitle: finished.story.title,
      workItemId: finished.workItem.id,
      role: finished.workItem.role,
      modelProfile: finished.workItem.modelProfile,
      toLane: followed,
    })
  }
  // ENG-FORGE-V4-11 — mirror the V4-10C follow Hold (same inputs/outcome the
  // Hold itself used inside runForgeFollow): a code-changing Smith run that
  // finished without a candidate commit never launches Assay against a
  // fallback base; the story is held instead.
  if (
    !followed &&
    !normalized.failed &&
    outcomeOk &&
    finished.workItem.role === 'builder' &&
    !commitHash
  ) {
    mirrorForgeSlack(slack, {
      event: 'lane-terminal',
      storyId: finished.workItem.storyId,
      storyTitle: finished.story.title,
      workItemId: finished.workItem.id,
      role: finished.workItem.role,
      modelProfile: finished.workItem.modelProfile,
      resultStatus: 'Hold',
      detail:
        'Smith produced no candidate commit for this code-changing run; story held so Assay never verifies a fallback base such as main.',
    })
  }

  // ENG-FORGE-V4-10B: publish an accepted candidate only after a CLEAN
  // terminal Assay (normalized 'Assay Failed' / Hold results are never
  // eligible). Runs in this outer Forge process; conflicts fail the story
  // closed into Hold and preserve the candidate.
  const publishOutcome = await runForgePublishAfterAssay({
    storyId: finished.workItem.storyId,
    finishedRole: finished.workItem.role,
    resultStatus: normalized.resultStatus,
    testsSummary: normalized.testsSummary,
  })
  if (publishOutcome?.kind === 'published') {
    console.log(
      'published accepted candidate',
      publishOutcome.candidateCommit.slice(0, 12),
      'to origin/main as',
      publishOutcome.publishedMainHash.slice(0, 12),
    )
  } else if (publishOutcome?.kind === 'publish-conflict') {
    console.log('publish conflict — story set to Hold:', publishOutcome.detail)
    mirrorForgeSlack(slack, {
      event: 'lane-terminal',
      storyId: finished.workItem.storyId,
      storyTitle: finished.story.title,
      workItemId: finished.workItem.id,
      role: finished.workItem.role,
      modelProfile: finished.workItem.modelProfile,
      resultStatus: 'Hold',
      detail: `publish conflict: ${publishOutcome.detail}`,
    })
  }

  // ENG-FORGE-V4-11 — every durable transition for this finish is complete;
  // flush the buffered Slack mirrors in causal order. Fail-open and never
  // throws, and there is no remaining Forge step it could delay.
  await flushForgeSlack(slack)
}

void main()
