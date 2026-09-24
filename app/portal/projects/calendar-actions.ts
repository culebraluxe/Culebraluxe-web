"use server"

import { randomUUID } from "node:crypto"

import { SqlCalendarRepository } from "@/legacy/db/calendar-service-repository"
import { getActingUser } from "@/lib/auth/get-acting-user"
import { getPortalSessionAdapter } from "@/lib/auth/portal-session"
import { captureServerError } from "@/lib/server-error-capture"
import { appServiceErrorSink } from "@/lib/service-error-sink"
import type { CatchUpCalendarEvent } from "@/lib/catchup/calendar-adapter"
import { CalendarService } from "@/legacy/services/calendar"
import type { CreateAppleCalendarEventRequest } from "@/legacy/services/calendar"
import type { ServiceContext } from "@/legacy/services/core"
import { AuthorizationService } from "@/legacy/services/entitlement"
import { resolveSecurityLevel } from "@/legacy/services/security"

export type LoadCatchUpScheduleResult =
  | { ok: true; events: CatchUpCalendarEvent[] }
  | { ok: false; message: string }

export type CreateAppleCalendarEventResult =
  | { ok: true; commandId: string; state: "queued" }
  | { ok: false; message: string }

function calendarService(): CalendarService {
  return new CalendarService(new SqlCalendarRepository(), {
    authorization: new AuthorizationService(),
    errors: appServiceErrorSink(),
  })
}

async function runContext(): Promise<ServiceContext> {
  const acting = await getActingUser(getPortalSessionAdapter())
  return {
    actor: { id: acting.appUserId, kind: "user" },
    correlationId: randomUUID(),
    principal: {
      appUserId: acting.appUserId,
      level: resolveSecurityLevel(acting.roleCodes),
      roleCodes: acting.roleCodes,
    },
  }
}

/** Authorized read for the normalized Catch-Up Schedule slice. */
export async function loadCatchUpScheduleAction(): Promise<LoadCatchUpScheduleResult> {
  try {
    const context = await runContext()
    const result = await calendarService().execute({
      operation: "calendar.list",
      payload: {},
      context,
    })
    if (!result.ok) return { ok: false, message: result.error.message }
    return { ok: true, events: result.value }
  } catch (error) {
    captureServerError("projects:catch-up-schedule", error, { level: "warn" })
    return { ok: false, message: "Schedule is temporarily unavailable." }
  }
}

/** Explicit user command. Vercel queues it durably; the trusted Mac gateway
 * performs the EventKit write on its next cycle. */
export async function createAppleCalendarEventAction(
  input: CreateAppleCalendarEventRequest,
): Promise<CreateAppleCalendarEventResult> {
  try {
    const context = await runContext()
    const result = await calendarService().execute({
      operation: "calendar.createAppleEvent",
      payload: input,
      context,
    })
    if (!result.ok) return { ok: false, message: result.error.message }
    return { ok: true, ...result.value }
  } catch (error) {
    captureServerError("projects:apple-calendar-create", error, { level: "warn" })
    return { ok: false, message: "Could not queue the Apple Calendar event." }
  }
}
