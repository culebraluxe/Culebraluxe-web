"use server"

import type { CatchUpCalendarEvent } from "@/lib/catchup/calendar-adapter"
import {
  rustApiCreateAppleCalendarEvent,
  rustApiRead,
} from "@/lib/rust-api/client"
import { captureServerError } from "@/lib/server-error-capture"

export type CreateAppleCalendarEventRequest = {
  title: string
  startAt: string
  endAt: string
  allDay?: boolean | null
  location?: string | null
  notes?: string | null
  alert?: boolean | null
}

export type LoadCatchUpScheduleResult =
  | { ok: true; events: CatchUpCalendarEvent[] }
  | { ok: false; message: string }

export type CreateAppleCalendarEventResult =
  | { ok: true; commandId: string; state: "queued" }
  | { ok: false; message: string }

export async function loadCatchUpScheduleAction(): Promise<LoadCatchUpScheduleResult> {
  try {
    const result = await rustApiRead<CatchUpCalendarEvent[]>("/v1/calendar")
    return { ok: true, events: result.value }
  } catch (error) {
    captureServerError("projects:catch-up-schedule", error, { level: "warn" })
    return { ok: false, message: "Schedule is temporarily unavailable." }
  }
}

export async function createAppleCalendarEventAction(
  input: CreateAppleCalendarEventRequest,
): Promise<CreateAppleCalendarEventResult> {
  try {
    const result = await rustApiCreateAppleCalendarEvent<{
      commandId: string
      state: "queued"
    }>({
      title: input.title,
      startAt: input.startAt,
      endAt: input.endAt,
      allDay: input.allDay ?? null,
      location: input.location ?? null,
      notes: input.notes ?? null,
      alert: input.alert ?? null,
    })
    return { ok: true, ...result.value }
  } catch (error) {
    captureServerError("projects:apple-calendar-create", error, { level: "warn" })
    return {
      ok: false,
      message:
        error instanceof Error
          ? error.message
          : "Could not queue the Apple Calendar event.",
    }
  }
}
