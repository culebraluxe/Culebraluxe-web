// ---------------------------------------------------------------------------
// CRM-14J — Canonical command wrappers: task.create / task.complete /
// task.cancel.
//
// Thin adapters over the existing canonical task services (db/tasks.ts,
// db/portal-writes.ts). The services own validation, mutation and conflict
// semantics (PortalWriteError); these handlers translate the envelope into the
// service call and map transport errors onto the CommandOutcome vocabulary —
// exactly the mapping the router performed before CRM-14J. Task commands are
// NOT receipt-backed today (same behavior as the pre-CRM-14J router: no
// idempotency receipts, so a re-submitted commandId re-executes). Registration
// happens in lib/commands/register.ts.
// ---------------------------------------------------------------------------

import { createTask as createCanonicalTask } from '../../../db/tasks'
import {
  completeTask as completeCanonicalTask,
  cancelTask as cancelCanonicalTask,
} from '../../../db/portal-writes'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'
import {
  TASK_CANCEL,
  TASK_COMPLETE,
  TASK_CREATE,
} from '../command-types'
import { failureResult, successResult } from '../result'

export { TASK_CANCEL, TASK_COMPLETE, TASK_CREATE }

export class CreateTaskCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const input = envelope.input as {
      title: string
      detail?: string
      personId?: string
      propertyId?: string
      dealId?: string
      dueAt?: string
      priority?: number
      taskKind?: 'human' | 'system'
    }
    try {
      const task = await createCanonicalTask({
        title: input.title,
        detail: input.detail,
        personId: input.personId,
        propertyId: input.propertyId,
        dealId: input.dealId,
        dueAt: input.dueAt,
        priority: input.priority,
        taskKind: input.taskKind,
      })
      return successResult(envelope, task.id)
    } catch (err) {
      return failureResult(
        envelope,
        err,
        null,
        'task.create failed',
      )
    }
  }
}

export class CompleteTaskCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const { taskId } = envelope.input as { taskId: string }
    try {
      await completeCanonicalTask(taskId)
      return successResult(envelope, taskId)
    } catch (err) {
      return failureResult(envelope, err, taskId, 'task.complete failed')
    }
  }
}

export class CancelTaskCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const { taskId } = envelope.input as { taskId: string }
    try {
      await cancelCanonicalTask(taskId)
      return successResult(envelope, taskId)
    } catch (err) {
      return failureResult(envelope, err, taskId, 'task.cancel failed')
    }
  }
}
