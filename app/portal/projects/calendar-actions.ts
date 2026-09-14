"use server"

import { getCatchUpCalendarEvents } from "@/db/catch-up-calendar"
import { getActingUser } from "@/lib/auth/get-acting-user"
import { getPortalSessionAdapter } from "@/lib/auth/portal-session"
import { captureServerError } from "@/lib/server-error-capture"
import type { CatchUpCalendarEvent } from "@/lib/catchup/calendar-adapter"

export type LoadCatchUpScheduleResult =
  | { ok: true; events: CatchUpCalendarEvent[] }
  | { ok: false; message: string }

/** Authorized read for the Catch-Up Schedule slice. Calendar facts stay
 * read-only; this action never mutates Apple Calendar or canonical showings. */
export async function loadCatchUpScheduleAction(): Promise<LoadCatchUpScheduleResult> {
  try {
    await getActingUser(getPortalSessionAdapter())
    return { ok: true, events: await getCatchUpCalendarEvents() }
  } catch (error) {
    captureServerError("projects:catch-up-schedule", error, { level: "warn" })
    return { ok: false, message: "Schedule is temporarily unavailable." }
  }
}
