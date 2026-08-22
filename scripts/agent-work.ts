// ---------------------------------------------------------------------------
// Agent work command — single-worker autonomous dispatch between the
// authoritative (PRODUCTION) Story Board and the coding agent.
//
//   pnpm agent:work                          claim the next configured Ready
//                                            item, VALIDATE the runtime
//                                            envelope, and dispatch it through
//                                            the existing AgentRuntimeAdapter
//                                            -> DeepSeekHarnessAdapter path
//                                            (runs to completion in-process)
//   pnpm agent:work --progress <workItemId> [--completion <n>]
//        [--note <text>] [--tests <text>]
//                                            persist live progress on the run
//                                            (completion / appended milestone
//                                            note / tests summary) + heartbeat
//   pnpm agent:work --finish <workItemId> --result <outcome> --completion <n>
//        [--notes <text>] [--commit <hash>] [--tests <text>]
//                                            record the finished run + mark
//                                            the work item Done
//   pnpm agent:work --error <workItemId> --error-text <text>
//        [--note <text>] [--completion <n>] [--tests <text>]
//                                            infra failure: terminate run as
//                                            Failed + mark the work item Error
//   pnpm agent:work --cancel <workItemId> [--note <text>]
//                                            cancel the run (result Cancelled,
//                                            story -> Hold) + work item
//                                            Cancelled
//   pnpm agent:work --recover [--stale-after <minutes>]
//                                            mark stale Claimed/Running items
//                                            terminal (run Failed, work Error)
//                                            and unblock the queue
//
// Contract (Story Board work-queue story):
//   - always targets the PRODUCTION control-plane database (refuses otherwise)
//   - claims AT MOST ONE Ready work item per invocation; exits cleanly with
//     "no work" when the queue is empty or another item is already active
//   - validates the runtime envelope (role / model profile / execution target /
//     execution policy / test mode) BEFORE Running; missing config fails fast
//     through the durable Error+Hold path and releases the global slot
//   - dispatches the claimed command through the shared invoker runtime phase
//     (the EXACT same path the debug driver uses) so the local DeepSeek Harness
//     launches without any story-specific launch command
//   - DEV remains the execution target; PROD remains control plane only
//   - NEVER loops into a second story in one invocation
//   - existing heartbeat/session/evidence/finalization behavior is unchanged
//   - commits repository changes itself; never pushes
// ---------------------------------------------------------------------------

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

  const workerId = process.env.AGENT_WORKER_ID ?? 'coding-agent'
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

  // HARD LAUNCH GUARD (ENG-20A): a claimed command with missing/invalid
  // execution configuration must NOT transition to Running. Fail fast through
  // the durable path (work Error + story Hold + global slot released) with a
  // concise actionable error instead of creating a zombie Running item.
  const launchError = validateAgentWorkLaunchConfig(workItem)
  if (launchError) {
    console.error('launch guard:', launchError)
    console.error(
      `work item ${workItem.id} marked Error; story ${story.id} set to Hold. ` +
        'Configure the command in the SDLC Command Console (role / model profile / execution target), then set the story back to Ready.',
    )
    await rejectAgentWorkConfiguration(workItem.id, launchError)
    process.exit(1)
  }

  // DEV remains the execution target; PROD remains control plane only. The
  // workspace .env.local must not resolve a DEV execution to the PROD DB.
  const target = parseExecutionEnvironment(process.env.EXECUTION_ENV, 'DEV')
  assertExecutionTargetSafe(target)
  verifyWorkspaceEnvFile(resolve(process.cwd()), target)

  // Dispatch the ALREADY-CLAIMED command through the EXISTING runtime path
  // (AgentRuntimeAdapter -> DeepSeekHarnessAdapter). No duplicate mechanism:
  // executeClaimedAgentCommand is the exact phase 2 of the debug driver.
  const work = new SqlAgentWorkRepository(() => interactiveSql as any)
  const runs = new SqlAgentRunRepository(() => interactiveSql as any)
  const registry = createAgentRuntimeRegistry()

  try {
    const result = await executeClaimedAgentCommand(workerId, claim, {
      work,
      runs,
      registry,
    })
    console.log('=== autonomous dispatch result ===')
    console.log('story:', result.storyId)
    console.log('role:', result.role, '| profile:', result.modelProfile, '| adapter:', result.runtimeAdapter)
    console.log('evidence:', result.evidence.resultStatus, result.evidence.completion + '%')
    console.log('external_run_id:', result.evidence.externalRunId)
    console.log('execution_target:', result.evidence.executionEnvironment ?? 'unset')
  } catch (e) {
    // Fail safely: terminalize the claimed item (Error, slot released) and
    // exit; the next scheduler cycle continues with the next eligible item.
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
  const { finishAgentWork, failAgentWork } = await import('../db/agent-work')

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

  const finished = await finishAgentWork(workItemId, {
    resultStatus,
    completion,
    notes: value(args, '--notes') ?? '',
    commitHash: value(args, '--commit') ?? null,
    testsSummary: value(args, '--tests') ?? null,
  })

  console.log('work item', finished.workItem.id, '->', finished.workItem.state)
  console.log('run result:', finished.run && (finished.run as { resultStatus?: string }).resultStatus)
  console.log('story status:', finished.story.status, '| completion:', finished.story.completion)
  if (value(args, '--commit')) {
    console.log('commit:', value(args, '--commit'))
  }
}

void main()
