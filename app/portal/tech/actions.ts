"use server"

import { withServerErrorCapture } from "@/lib/error-capture-seam"
import { rustApiTechCommand } from "@/lib/rust-api/client"

type TechCommandResult = {
  ok: boolean
  message: string
  data?: Record<string, unknown> | null
}

async function command(body: Record<string, unknown>): Promise<TechCommandResult> {
  return (await rustApiTechCommand(body)) as TechCommandResult
}

async function setActiveWorkActionHandler(formData: FormData): Promise<void> {
  const storyId = String(formData.get("storyId") ?? "").trim()
  if (!storyId) return
  await command({
    action: "setActiveWork",
    storyId,
    active: formData.get("active") === "true",
  })
}

export const setActiveWorkAction = withServerErrorCapture(
  "portal/tech/actions.setActiveWorkAction",
  setActiveWorkActionHandler,
)

async function clearWorkBenchActionHandler(): Promise<{
  ok: boolean
  cleared?: number
  error?: string
}> {
  try {
    const result = await command({ action: "clearWorkbench" })
    return {
      ok: result.ok,
      cleared: Number(result.data?.cleared ?? 0),
      error: result.ok ? undefined : result.message,
    }
  } catch (error) {
    return { ok: false, error: String((error as Error)?.message ?? error) }
  }
}

export const clearWorkBenchAction = withServerErrorCapture(
  "portal/tech/actions.clearWorkBenchAction",
  clearWorkBenchActionHandler,
)

async function moveStoryBucketActionHandler(
  cardId: string,
  from: string,
  to: string,
): Promise<{ ok: boolean; error?: string; note?: string }> {
  try {
    const result = await command({
      action: "moveStoryBucket",
      storyId: String(cardId ?? "").trim(),
      source: String(from ?? ""),
      target: String(to ?? ""),
    })
    return {
      ok: result.ok,
      error: result.ok ? undefined : result.message,
      note:
        typeof result.data?.note === "string"
          ? result.data.note
          : result.ok
            ? result.message
            : undefined,
    }
  } catch (error) {
    const message = String((error as Error)?.message ?? error)
    return { ok: false, error: `the write threw: ${message}` }
  }
}

export const moveStoryBucketAction = withServerErrorCapture(
  "portal/tech/actions.moveStoryBucketAction",
  moveStoryBucketActionHandler,
)

async function sendEngineBatchActionHandler(): Promise<{
  ok: boolean
  queued: number
  failed: Array<{ storyId: string; error: string }>
  batchId?: string
  error?: string
}> {
  try {
    const result = await command({ action: "launchFlight" })
    const failed = Array.isArray(result.data?.failed)
      ? (result.data?.failed as Array<{ storyId: string; error: string }>)
      : []
    return {
      ok: result.ok,
      queued: Number(result.data?.queued ?? 0),
      failed,
      batchId:
        typeof result.data?.batchId === "string"
          ? result.data.batchId
          : undefined,
      error: result.ok ? undefined : result.message,
    }
  } catch (error) {
    return {
      ok: false,
      queued: 0,
      failed: [],
      error: String((error as Error)?.message ?? error),
    }
  }
}

export const sendEngineBatchAction = withServerErrorCapture(
  "portal/tech/actions.sendEngineBatchAction",
  sendEngineBatchActionHandler,
)

async function scheduleEngineBatchActionHandler(
  scheduledForIso: string,
  label?: string,
): Promise<{
  ok: boolean
  batchId?: string
  stories?: number
  scheduledFor?: string
  error?: string
}> {
  try {
    const result = await command({
      action: "scheduleFlight",
      scheduledFor: String(scheduledForIso ?? ""),
      label: label?.trim() || null,
    })
    return {
      ok: result.ok,
      batchId:
        typeof result.data?.batchId === "string"
          ? result.data.batchId
          : undefined,
      stories:
        result.data?.stories == null
          ? undefined
          : Number(result.data.stories),
      scheduledFor:
        typeof result.data?.scheduledFor === "string"
          ? result.data.scheduledFor
          : undefined,
      error: result.ok ? undefined : result.message,
    }
  } catch (error) {
    return {
      ok: false,
      error: String((error as Error)?.message ?? error),
    }
  }
}

export const scheduleEngineBatchAction = withServerErrorCapture(
  "portal/tech/actions.scheduleEngineBatchAction",
  scheduleEngineBatchActionHandler,
)

async function cancelScheduledBatchActionHandler(
  batchId: string,
): Promise<{ ok: boolean; cancelled?: number; error?: string }> {
  try {
    const result = await command({
      action: "cancelFlight",
      batchId: String(batchId ?? "").trim(),
    })
    return {
      ok: result.ok,
      cancelled:
        result.data?.cancelled == null
          ? undefined
          : Number(result.data.cancelled),
      error: result.ok ? undefined : result.message,
    }
  } catch (error) {
    return {
      ok: false,
      error: String((error as Error)?.message ?? error),
    }
  }
}

export const cancelScheduledBatchAction = withServerErrorCapture(
  "portal/tech/actions.cancelScheduledBatchAction",
  cancelScheduledBatchActionHandler,
)

export type ScopedRunTarget = "scout" | "architect" | "lead"
export type LaunchIntent = "SOLO" | "SMITH" | "SPLIT" | "HOLD" | null

async function queueScopedRunActionHandler(input: {
  storyId: string
  stopAfter: ScopedRunTarget
  launchIntent?: LaunchIntent
}): Promise<{ ok: boolean; error?: string }> {
  try {
    const result = await command({
      action: "scopedRun",
      storyId: String(input.storyId ?? "").trim(),
      stopAfter: input.stopAfter,
      launchIntent: input.launchIntent ?? null,
    })
    return {
      ok: result.ok,
      error: result.ok ? undefined : result.message,
    }
  } catch (error) {
    return {
      ok: false,
      error: String((error as Error)?.message ?? error),
    }
  }
}

export const queueScopedRunAction = withServerErrorCapture(
  "portal/tech/actions.queueScopedRunAction",
  queueScopedRunActionHandler,
)
