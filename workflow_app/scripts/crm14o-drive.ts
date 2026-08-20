// ---------------------------------------------------------------------------
// DEV-ONLY CRM-14O runtime driver. NOT part of the production build and never
// committed. It exercises the LIVE DEV RE_supermodel instance strictly through
// the application/runtime seams:
//
//   - task completion  : db/portal-writes.completeTask (canonical) +
//                        workflow_app/task-completion.completeWorkflowTask (engine)
//   - closing date     : db/deal-closing-date.setDealClosingDate (command + receipt)
//   - timer reschedule : workflow_app/closing-timer.reconcileClosingTimer
//   - materialization  : workflow_app/reconcile.reconcileWorkflows
//
// No direct engine-table mutation. Read commands are allowed for inspection.
// ---------------------------------------------------------------------------

import { randomUUID } from 'node:crypto'

import { sql } from '../../db/client'
import { completeTask as completeCanonicalTask } from '../../db/portal-writes'
import { setDealClosingDate } from '../../db/deal-closing-date'
import { getDealWorkflowFacts } from '../facts'
import { reconcileWorkflows } from '../reconcile'
import { reconcileClosingTimer } from '../closing-timer'
import { completeWorkflowTask } from '../task-completion'
import { inspectInstance, listCommandReceipts } from '../diagnostics'

const DEFAULT_INSTANCE = 'cd7f2da0-9e26-4267-8c16-3e3df0bf0f2c'
const INSTANCE_ID = process.env.INSTANCE_ID ?? DEFAULT_INSTANCE
const USER_ID = process.env.USER_ID ?? 'crm14o-driver'

function log(label: string, value: unknown): void {
  console.log(`${label}: ${JSON.stringify(value, null, 2)}`)
}

async function snapshot(): Promise<void> {
  const detail = await inspectInstance(INSTANCE_ID)
  if (!detail) {
    console.log(`No instance ${INSTANCE_ID}`)
    return
  }

  const dealId = detail.subjectId
  const facts = dealId ? await getDealWorkflowFacts(dealId) : null
  const receipts = await listCommandReceipts()

  const tokensByNode = new Map<string, Array<{ id: string; status: string; outcome: string | null }>>()
  for (const t of detail.tokens) {
    const list = tokensByNode.get(t.nodeId) ?? []
    list.push({ id: t.id, status: t.status, outcome: t.outcome })
    tokensByNode.set(t.nodeId, list)
  }

  const activeTasks = detail.tasks.filter((t) =>
    ['ready', 'reserved', 'in_progress'].includes(t.status),
  )
  const engineTaskView = activeTasks.map((t) => {
    const token = detail.tokens.find((tk) => tk.id === t.tokenId)
    const corr = detail.correlations.find((c) => c.workflowTaskId === t.id)
    return {
      engineTaskId: t.id,
      name: t.name,
      status: t.status,
      nodeId: token?.nodeId ?? null,
      canonicalTaskId: corr?.applicationTaskId ?? null,
      canonicalStatus: corr?.applicationTaskStatus ?? null,
    }
  })

  log('instance', {
    id: detail.instanceId,
    definitionKey: detail.definitionKey,
    definitionVersion: detail.definitionVersion,
    status: detail.status,
    outcome: detail.outcome,
    subjectType: detail.subjectType,
    subjectId: detail.subjectId,
    startedAt: detail.startedAt,
    endedAt: detail.endedAt,
    counts: {
      activeTokens: detail.activeTokenCount,
      tasks: detail.taskCount,
      events: detail.eventCount,
    },
  })

  log('deal', {
    id: dealId,
    stage: facts?.stage ?? null,
    closingDate: facts?.closingDate ?? null,
    closingDateScheduled: facts?.closingDateScheduled ?? null,
    financingApplicable: facts?.financingApplicable ?? null,
    closingConfirmationRequired: facts?.closingConfirmationRequired ?? null,
    requiresRegistryFollowup: facts?.requiresRegistryFollowup ?? null,
    inspectionApplicable: facts?.inspectionApplicable ?? null,
    insuranceApplicable: facts?.insuranceApplicable ?? null,
  })

  log('tokens', detail.tokens.map((t) => ({
    id: t.id,
    parent: t.parentTokenId,
    nodeId: t.nodeId,
    label: detail.nodeLabels[t.nodeId] ?? t.nodeId,
    status: t.status,
    outcome: t.outcome,
    required: t.required,
  })))

  log('activeEngineTasks', engineTaskView)
  log('jobs', detail.jobs)
  log('commands', detail.commands)
  log('pendingReceipts', receipts.filter((r) => r.outcome === 'pending'))
  log('eventCount', detail.events.length)
  log('eventTypes', [...new Set(detail.events.map((e) => e.eventType))])
}

async function findBranchTask(
  detail: NonNullable<Awaited<ReturnType<typeof inspectInstance>>>,
  nodeId: string,
): Promise<{ engineTaskId: string; canonicalTaskId: string; name: string } | null> {
  const token = detail.tokens.find((t) => t.nodeId === nodeId && t.status === 'active')
  if (!token) return null
  const task = detail.tasks.find(
    (t) => t.tokenId === token.id && ['ready', 'reserved', 'in_progress'].includes(t.status),
  )
  if (!task) return null
  const corr = detail.correlations.find((c) => c.workflowTaskId === task.id)
  if (!corr?.applicationTaskId) {
    return { engineTaskId: task.id, canonicalTaskId: '', name: task.name }
  }
  return { engineTaskId: task.id, canonicalTaskId: corr.applicationTaskId, name: task.name }
}

async function completeBranch(nodeId: string, transitionName?: string): Promise<void> {
  // Materialize any freshly-created engine tasks into canonical tasks first
  // (idempotent) so a brand-new blocker/loop task always has a correlation.
  const before = await reconcileWorkflows()
  if (before.materializedTasks > 0) log('materialized', before)

  const detail = await inspectInstance(INSTANCE_ID)
  if (!detail) throw new Error(`No instance ${INSTANCE_ID}`)
  const target = await findBranchTask(detail, nodeId)
  if (!target) {
    throw new Error(`No active task at node '${nodeId}'`)
  }
  log('completing', { nodeId, ...target, transition: transitionName ?? '(default)' })

  if (target.canonicalTaskId) {
    await completeCanonicalTask(target.canonicalTaskId)
    log('canonical', { taskId: target.canonicalTaskId, status: 'completed' })
  } else {
    throw new Error(`Engine task ${target.engineTaskId} has no canonical correlation after reconcile`)
  }

  await completeWorkflowTask({
    applicationTaskId: target.canonicalTaskId,
    userId: USER_ID,
    transitionName,
  })
  log('engine', { taskId: target.engineTaskId, status: 'completed' })
}

async function setDate(closingDate: string): Promise<void> {
  const detail = await inspectInstance(INSTANCE_ID)
  if (!detail) throw new Error(`No instance ${INSTANCE_ID}`)
  const dealId = detail.subjectId
  if (!dealId) throw new Error('Instance has no deal subject')
  const commandId = randomUUID()
  const result = await setDealClosingDate({ dealId, closingDate, commandId })
  log('setClosingDate', { commandId, closingDate, result })
}

async function reconcileTimer(): Promise<void> {
  const detail = await inspectInstance(INSTANCE_ID)
  if (!detail) throw new Error(`No instance ${INSTANCE_ID}`)
  const dealId = detail.subjectId
  const facts = dealId ? await getDealWorkflowFacts(dealId) : null
  const result = await reconcileClosingTimer(INSTANCE_ID, facts?.closingDate ?? null)
  log('reconcileTimer', result)
}

async function main(): Promise<void> {
  const [cmd, ...args] = process.argv.slice(2)
  switch (cmd) {
    case 'snapshot':
      await snapshot()
      break
    case 'complete':
      if (!args[0]) throw new Error('usage: complete <nodeId> [transitionName]')
      await completeBranch(args[0], args[1])
      await snapshot()
      break
    case 'set-date':
      if (!args[0]) throw new Error('usage: set-date <iso-date>')
      await setDate(args[0])
      break
    case 'timer':
      await reconcileTimer()
      break
    case 'reconcile':
      log('reconcile', await reconcileWorkflows())
      break
    case 'facts':
      log('facts', await getDealWorkflowFacts((await inspectInstance(INSTANCE_ID))?.subjectId ?? ''))
      break
    default:
      console.log('commands: snapshot | complete <node> [transition] | set-date <iso> | timer | reconcile | facts')
  }
}

main().catch((err) => {
  console.error('FAIL:', err instanceof Error ? err.message : err)
  process.exit(1)
})
