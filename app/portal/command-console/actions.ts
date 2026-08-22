'use server'

import { revalidatePath } from 'next/cache'

import { PortalWriteError } from '@/lib/portal-write-error'
import {
  cancelAgentWork,
  enqueueAgentWorkCommand,
  pauseAgentWork,
  resumeAgentWork,
} from '@/db/agent-work'

export type ConsoleActionResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: string; code: string }

const EXECUTION_POLICIES = ['Unattended OK', 'Daytime Only', 'Human Gate', 'Manual Only']
const ROLES = ['architect', 'builder', 'reviewer', 'verifier']
const MODEL_PROFILES = ['architect-pro', 'builder-flash', 'reviewer', 'local-builder']
const EXECUTION_ENVIRONMENTS = ['DEV', 'PROD', 'TEST', 'LOCAL']
const TEST_MODES = ['SCOPED', 'FULL', 'NONE']

function result<T>(fn: () => Promise<T>): Promise<ConsoleActionResult<T>> {
  return fn().then(
    (data) => ({ ok: true as const, data }),
    (e) => {
      if (e instanceof PortalWriteError) {
        return { ok: false as const, error: e.message, code: e.code }
      }
      return {
        ok: false as const,
        error: String((e as Error)?.message ?? e),
        code: 'unknown',
      }
    },
  )
}

/**
 * Queue a durable agent_work_item for a story with the logical command
 * envelope (role / model profile / optional instructions / execution policy).
 * The canonical story specification is NEVER copied here — it is resolved from
 * storyboard_story at execution time. This is the console's only command
 * creation path; no second command table exists.
 */
export async function queueCommandAction(input: {
  storyId: string
  role: string
  modelProfile: string
  specialInstructions: string
  executionPolicy: string
  executionEnvironment: string
  testMode: string
}): Promise<ConsoleActionResult<{ workItemId: string }>> {
  if (!input.storyId) return { ok: false, error: 'storyId is required', code: 'validation' }
  if (!ROLES.includes(input.role)) {
    return { ok: false, error: `unknown role: ${input.role}`, code: 'validation' }
  }
  if (!MODEL_PROFILES.includes(input.modelProfile)) {
    return { ok: false, error: `unknown model profile: ${input.modelProfile}`, code: 'validation' }
  }
  if (!EXECUTION_POLICIES.includes(input.executionPolicy)) {
    return { ok: false, error: `unknown execution policy: ${input.executionPolicy}`, code: 'validation' }
  }
  if (!EXECUTION_ENVIRONMENTS.includes(input.executionEnvironment)) {
    return { ok: false, error: `unknown execution environment: ${input.executionEnvironment}`, code: 'validation' }
  }
  if (!TEST_MODES.includes(input.testMode)) {
    return { ok: false, error: `unknown test mode: ${input.testMode}`, code: 'validation' }
  }
  if (input.specialInstructions.length > 4000) {
    return { ok: false, error: 'special instructions too long (max 4000 chars)', code: 'validation' }
  }

  return result(async () => {
    const item = await enqueueAgentWorkCommand({
      storyId: input.storyId,
      role: input.role,
      modelProfile: input.modelProfile,
      // The runtime test execution mode is machine-visible and authoritative:
      // it is carried in the durable special_instructions envelope as a reserved
      // directive the runtime parses and strips. It OUTRANKS contradictory
      // story prose.
      specialInstructions:
        `[runtime test-mode: ${input.testMode}] ${input.specialInstructions.trim()}`.trim(),
      executionPolicy: input.executionPolicy,
      executionEnvironment: input.executionEnvironment,
    })
    revalidatePath('/portal/command-console')
    return { workItemId: item.id }
  })
}

/** Cancel a running/claimed command (terminal, never success). */
export async function cancelCommandAction(
  workItemId: string,
): Promise<ConsoleActionResult<{ workItemId: string }>> {
  return result(async () => {
    await cancelAgentWork(workItemId)
    revalidatePath('/portal/command-console')
    return { workItemId }
  })
}

/** Pause a Running command (preserves assignment). */
export async function pauseCommandAction(
  workItemId: string,
): Promise<ConsoleActionResult<{ workItemId: string }>> {
  return result(async () => {
    await pauseAgentWork(workItemId)
    revalidatePath('/portal/command-console')
    return { workItemId }
  })
}

/** Resume a Paused command (same logical attempt). */
export async function resumeCommandAction(
  workItemId: string,
): Promise<ConsoleActionResult<{ workItemId: string }>> {
  return result(async () => {
    await resumeAgentWork(workItemId)
    revalidatePath('/portal/command-console')
    return { workItemId }
  })
}
