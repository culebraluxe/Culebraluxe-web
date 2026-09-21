/**
 * The workflow engine host.
 *
 * This used to spawn a `re-workflow` binary once per operation: a process, a pool and a runtime per call, no share of
 * the server's identity resolution or error capture, and an `RE_WORKFLOW_BIN` that had to be set correctly or nothing
 * worked at all. The engine is now served by the Rust API itself (`server/src/api/engine.rs`), so these are ordinary
 * calls to the server that is already running - and `RE_WORKFLOW_BIN` is gone.
 *
 * The functions are typed and async, rather than the old argv-in/string-out wrapper, because that synchronous signature
 * was the reason a process was spawned in the first place: `spawnSync` was the only way to get an answer without
 * awaiting. Callers now await, which is why they can talk to the server directly.
 */
import { rustApiEngineCommand } from '@/lib/rust-api/client'

export async function startResidentialTransaction(
  subject: 'deal' | 'contract',
  id: string,
): Promise<{ instanceId: string; started: boolean }> {
  const { value } = await rustApiEngineCommand<{ instanceId: string; started: boolean }>(
    '/v1/engine/transactions',
    { subject, id },
  )
  return { instanceId: value.instanceId, started: value.started }
}

export async function reconcileTimerNode(
  instance: string,
  node: string,
  date: string | null,
): Promise<string> {
  const { value } = await rustApiEngineCommand<{ node: string; applied: string }>(
    '/v1/engine/timers/reconcile',
    { instance, node, ...(date ? { date } : {}) },
  )
  return value.applied
}

export async function completeApplicationTask(
  taskId: string,
  userId: string,
  transitionName?: string,
): Promise<string> {
  const { value } = await rustApiEngineCommand<{ result: string }>('/v1/engine/tasks/complete', {
    task: taskId,
    user: userId,
    ...(transitionName ? { transition: transitionName } : {}),
  })
  return value.result
}

export async function completeEngineTask(
  taskId: string,
  userId: string,
  transitionName?: string,
): Promise<void> {
  await rustApiEngineCommand('/v1/engine/tasks/complete', {
    task: taskId,
    user: userId,
    kind: 'engine',
    ...(transitionName ? { transition: transitionName } : {}),
  })
}

export async function reclaimStaleJobs(batch = 50): Promise<number> {
  const { value } = await rustApiEngineCommand<{ reclaimed: number }>('/v1/engine/reclaim', {
    batch,
  })
  return value.reclaimed
}

export async function reclaimStaleJobsForInstance(instance: string): Promise<number> {
  const { value } = await rustApiEngineCommand<{ reclaimed: number }>('/v1/engine/reclaim', {
    instance,
  })
  return value.reclaimed
}

