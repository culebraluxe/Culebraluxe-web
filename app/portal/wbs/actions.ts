"use server"

import { randomUUID } from "node:crypto"

import {
  RustApiError,
  rustApiCreateWbs,
  rustApiQueueAppleReminder,
  rustApiUpdateWbs,
} from "@/lib/rust-api/client"

export type WbsCategoryId =
  | "clients"
  | "contracts"
  | "properties"
  | "media"
  | "marketing"
  | "accounting"
  | "management"

type RustWbsItem = {
  id: string
}

export type WbsActionResult =
  | { ok: true; id: string }
  | { ok: false; code: string; message: string }

export type AppleReminderActionResult =
  | { ok: true; commandId: string; state: "queued" }
  | { ok: false; code: string; message: string }

function failure(error: unknown, fallback: string): WbsActionResult {
  return {
    ok: false,
    code: error instanceof RustApiError ? error.code : "auth",
    message: error instanceof Error ? error.message : fallback,
  }
}

export async function createWbsItemAction(input: {
  title: string
  category: WbsCategoryId
  projectId?: string | null
}): Promise<WbsActionResult> {
  try {
    const result = await rustApiCreateWbs<RustWbsItem>({
      id: randomUUID(),
      title: input.title,
      category: input.category,
      projectId: input.projectId ?? null,
    })
    return { ok: true, id: result.value.id }
  } catch (error) {
    return failure(error, "Could not create follow-up.")
  }
}

export async function completeWbsItemAction(id: string): Promise<WbsActionResult> {
  try {
    const result = await rustApiUpdateWbs<RustWbsItem>(id, { status: "done" })
    return { ok: true, id: result.value.id }
  } catch (error) {
    return failure(error, "Could not complete follow-up.")
  }
}

export async function dismissWbsItemAction(id: string): Promise<WbsActionResult> {
  try {
    const result = await rustApiUpdateWbs<RustWbsItem>(id, {
      status: "dismissed",
    })
    return { ok: true, id: result.value.id }
  } catch (error) {
    return failure(error, "Could not dismiss follow-up.")
  }
}

export async function updateWbsItemAction(input: {
  id: string
  title?: string
  status?: "open" | "doing" | "done" | "dismissed"
  dueAt?: string | null
  owner?: string | null
  notes?: string
}): Promise<WbsActionResult> {
  try {
    const body: Record<string, unknown> = {}
    if (input.title !== undefined) body.title = input.title
    if (input.status !== undefined) body.status = input.status
    if (input.dueAt !== undefined) body.dueAt = input.dueAt
    if (input.owner !== undefined) body.owner = input.owner
    if (input.notes !== undefined) body.notes = input.notes
    const result = await rustApiUpdateWbs<RustWbsItem>(input.id, body)
    return { ok: true, id: result.value.id }
  } catch (error) {
    return failure(error, "Could not update work item.")
  }
}

export async function queueAppleReminderForWbsAction(
  id: string,
  options?: { alert?: boolean },
): Promise<AppleReminderActionResult> {
  try {
    const result = await rustApiQueueAppleReminder<{
      commandId: string
      state: "queued"
    }>(id, { alert: options?.alert ?? false })
    return { ok: true, ...result.value }
  } catch (error) {
    return {
      ok: false,
      code: error instanceof RustApiError ? error.code : "apple_gateway",
      message:
        error instanceof Error
          ? error.message
          : "Could not queue Apple Reminder.",
    }
  }
}
